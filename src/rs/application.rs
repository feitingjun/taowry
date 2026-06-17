//! Application 核心模块
//!
//! 包含 Application 结构体和事件循环实现，
//! 管理窗口、菜单、托盘的生命周期。

use crate::channel;
use crate::protocol::ProtocolState;
use crate::rpc::{parse_ipc_message, RpcMessageType, RpcState};
use crate::window::{load_tao_icon, load_tray_icon, BrowserWindow};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tao::dpi::{LogicalPosition, LogicalSize, Size};
use tao::event_loop::{EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::monitor::MonitorHandle;
use tao::window::{Fullscreen, Theme, WindowBuilder, WindowId};
use tray_icon::menu::{
    accelerator::Accelerator, CheckMenuItem, ContextMenu, IsMenuItem, Menu, MenuEvent, MenuItem,
    PredefinedMenuItem, Submenu,
};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use wry::{DragDropEvent, NewWindowResponse, PageLoadEvent, Rect, WebViewBuilder};

/// 事件循环中的用户事件类型
pub enum Action {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}

/// 被管理的菜单类型（顶层菜单或子菜单）
#[derive(Clone)]
pub enum ManagedMenu {
    Menu(Menu),
    Submenu(Submenu),
}

/// 被管理的菜单项类型
#[derive(Clone)]
pub enum ManagedMenuItem {
    Normal(MenuItem),
    Check(CheckMenuItem),
    Predefined(PredefinedMenuItem),
    Submenu(Submenu),
}

/// 应用实例，管理所有窗口、菜单、托盘的生命周期
pub struct Application {
    pub windows: HashMap<String, BrowserWindow>,
    pub menus: HashMap<String, ManagedMenu>,
    pub menu_items: HashMap<String, ManagedMenuItem>,
    pub trays: HashMap<String, TrayIcon>,
    pub proxy: Option<EventLoopProxy<Action>>,
    menu_counter: u64,
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            menus: HashMap::new(),
            menu_items: HashMap::new(),
            trays: HashMap::new(),
            proxy: None,
            menu_counter: 0,
        }
    }

    /// 创建事件循环（必须在主线程调用，macOS 要求）
    pub fn create_event_loop() -> tao::event_loop::EventLoop<Action> {
        EventLoopBuilder::<Action>::with_user_event().build()
    }

    pub fn get_window(&self, label: &str) -> Option<&BrowserWindow> {
        self.windows.get(label)
    }

    pub fn close_window(&mut self, label: &str) -> bool {
        self.windows.remove(label).is_some()
    }

    pub fn label_for_window_id(&self, window_id: WindowId) -> Option<String> {
        self.windows
            .iter()
            .find(|(_, window)| window.id() == window_id)
            .map(|(label, _)| label.clone())
    }

    /// 创建新的窗口，包含 tao 窗口和 wry WebView
    pub fn create_new_window(
        &mut self,
        event_loop: &EventLoopWindowTarget<Action>,
        label: String,
        mut window_builder: WindowBuilder,
        data: &Value,
    ) -> Result<String, String> {
        if self.windows.contains_key(&label) {
            return Err(format!("window '{}' already exists", label));
        }

        if window_builder.window.inner_size.is_none() {
            window_builder = window_builder.with_inner_size(LogicalSize::new(800.0, 600.0));
        }
        let size = window_builder
            .window
            .inner_size
            .ok_or_else(|| "missing window size".to_string())?;
        let window = window_builder
            .build(event_loop)
            .map_err(|error| format!("failed to create window '{}': {}", label, error))?;

        let mut webview_builder = WebViewBuilder::new().with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size,
        });
        let rpc_state = Arc::new(Mutex::new(RpcState::new()));
        let protocol_state = Arc::new(Mutex::new(ProtocolState::new()));
        webview_builder = apply_webview_options(
            label.clone(),
            webview_builder,
            data,
            rpc_state.clone(),
            protocol_state.clone(),
        )?;

        let webview = webview_builder
            .build_as_child(&window)
            .map_err(|error| format!("failed to create webview '{}': {}", label, error))?;
        let id = window.id();
        let id_string = format!("{:?}", id);
        self.windows.insert(
            label.clone(),
            BrowserWindow::new(label, window, webview, id, rpc_state, protocol_state),
        );
        Ok(id_string)
    }

    /// 创建菜单栏，支持嵌套子菜单
    pub fn create_menu(&mut self, label: String, data: &Value) -> Result<(), String> {
        if self.menus.contains_key(&label) {
            return Err(format!("menu '{}' already exists", label));
        }
        let menu = Menu::with_id(label.clone());
        self.menus
            .insert(label.clone(), ManagedMenu::Menu(menu.clone()));
        if let Some(items) = data.as_array() {
            for (index, item) in items.iter().enumerate() {
                let built = self.build_menu_item(&label, index, item)?;
                append_item_to_menu(&ManagedMenu::Menu(menu.clone()), &built)?;
            }
        }
        Ok(())
    }

    pub fn append_menu_item(&mut self, menu_label: &str, data: &Value) -> Result<String, String> {
        let index = self.menu_counter as usize;
        let menu = self
            .menus
            .get(menu_label)
            .cloned()
            .ok_or_else(|| format!("menu '{}' does not exist", menu_label))?;
        let item = self.build_menu_item(menu_label, index, data)?;
        let item_id = managed_item_id(&item);
        append_item_to_menu(&menu, &item)?;
        Ok(item_id)
    }

    /// 创建系统托盘图标
    pub fn create_tray(&mut self, label: String, data: &Value) -> Result<(), String> {
        if self.trays.contains_key(&label) {
            return Err(format!("tray '{}' already exists", label));
        }
        let mut builder = TrayIconBuilder::new().with_id(label.clone());
        if let Some(icon_path) = data.get("icon").and_then(Value::as_str) {
            builder = builder.with_icon(load_tray_icon(icon_path)?);
        }
        if let Some(tooltip) = data.get("tooltip").and_then(Value::as_str) {
            builder = builder.with_tooltip(tooltip);
        }
        if let Some(title) = data.get("title").and_then(Value::as_str) {
            builder = builder.with_title(title);
        }
        if let Some(temp_dir_path) = data.get("tempDirPath").and_then(Value::as_str) {
            builder = builder.with_temp_dir_path(temp_dir_path);
        }
        if let Some(icon_is_template) = data.get("iconAsTemplate").and_then(Value::as_bool) {
            builder = builder.with_icon_as_template(icon_is_template);
        }
        if let Some(menu_on_left_click) = data.get("menuOnLeftClick").and_then(Value::as_bool) {
            builder = builder.with_menu_on_left_click(menu_on_left_click);
        }
        if let Some(menu_label) = data.get("menu").and_then(Value::as_str) {
            let menu = self
                .menus
                .get(menu_label)
                .ok_or_else(|| format!("menu '{}' does not exist", menu_label))?;
            builder = builder.with_menu(menu.as_context_menu()?);
        }
        let tray = builder
            .build()
            .map_err(|error| format!("failed to create tray '{}': {}", label, error))?;
        self.trays.insert(label, tray);
        Ok(())
    }

    pub fn remove_tray(&mut self, label: &str) -> Result<(), String> {
        self.trays
            .remove(label)
            .map(|_| ())
            .ok_or_else(|| format!("tray '{}' does not exist", label))
    }

    pub fn set_tray_icon(&self, label: &str, icon_path: Option<&str>) -> Result<(), String> {
        let tray = self
            .trays
            .get(label)
            .ok_or_else(|| format!("tray '{}' does not exist", label))?;
        let icon = match icon_path {
            Some(path) => Some(load_tray_icon(path)?),
            None => None,
        };
        tray.set_icon(icon)
            .map_err(|error| format!("failed to set tray icon '{}': {}", label, error))
    }

    pub fn set_tray_menu(&self, label: &str, menu_label: Option<&str>) -> Result<(), String> {
        let tray = self
            .trays
            .get(label)
            .ok_or_else(|| format!("tray '{}' does not exist", label))?;
        let menu = match menu_label {
            Some(menu_label) => Some(
                self.menus
                    .get(menu_label)
                    .ok_or_else(|| format!("menu '{}' does not exist", menu_label))?
                    .as_context_menu()?,
            ),
            None => None,
        };
        tray.set_menu(menu);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn set_dock_menu(&self, menu_label: &str) -> Result<(), String> {
        let menu = self
            .menus
            .get(menu_label)
            .ok_or_else(|| format!("menu '{}' does not exist", menu_label))?;
        let ns_menu = menu.as_context_menu()?.ns_menu();
        crate::dock::platform::set_dock_menu(ns_menu);
        Ok(())
    }

    pub fn set_application_menu(&self, menu_label: &str) -> Result<(), String> {
        let menu = self
            .menus
            .get(menu_label)
            .ok_or_else(|| format!("menu '{}' does not exist", menu_label))?
            .as_root_menu()?;

        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = menu;
            Err("application menu is only supported on macOS by muda".to_string())
        }
    }

    pub fn set_window_menu(&self, window_label: &str, menu_label: &str) -> Result<(), String> {
        let window = self
            .windows
            .get(window_label)
            .ok_or_else(|| format!("window '{}' does not exist", window_label))?;
        let menu = self
            .menus
            .get(menu_label)
            .ok_or_else(|| format!("menu '{}' does not exist", menu_label))?
            .as_root_menu()?;

        #[cfg(target_os = "windows")]
        {
            use tao::platform::windows::WindowExtWindows;
            menu.init_for_hwnd(window.window.hwnd())
                .map_err(|error| format!("failed to set window menu: {}", error))
        }

        #[cfg(target_os = "macos")]
        {
            let _ = window;
            menu.init_for_nsapp();
            Ok(())
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            let _ = window;
            let _ = menu;
            Err("window menu is not implemented for this platform".to_string())
        }
    }

    pub fn set_menu_item_enabled(&self, item_id: &str, enabled: bool) -> Result<(), String> {
        match self.menu_items.get(item_id) {
            Some(ManagedMenuItem::Normal(item)) => item.set_enabled(enabled),
            Some(ManagedMenuItem::Check(item)) => item.set_enabled(enabled),
            Some(ManagedMenuItem::Submenu(item)) => item.set_enabled(enabled),
            Some(ManagedMenuItem::Predefined(_)) => {
                return Err("predefined menu item enabled state cannot be changed".to_string())
            }
            None => return Err(format!("menu item '{}' does not exist", item_id)),
        }
        Ok(())
    }

    pub fn set_menu_item_text(&self, item_id: &str, text: &str) -> Result<(), String> {
        match self.menu_items.get(item_id) {
            Some(ManagedMenuItem::Normal(item)) => item.set_text(text),
            Some(ManagedMenuItem::Check(item)) => item.set_text(text),
            Some(ManagedMenuItem::Submenu(item)) => item.set_text(text),
            Some(ManagedMenuItem::Predefined(item)) => item.set_text(text),
            None => return Err(format!("menu item '{}' does not exist", item_id)),
        }
        Ok(())
    }

    pub fn set_menu_item_checked(&self, item_id: &str, checked: bool) -> Result<(), String> {
        match self.menu_items.get(item_id) {
            Some(ManagedMenuItem::Check(item)) => {
                item.set_checked(checked);
                Ok(())
            }
            Some(_) => Err(format!("menu item '{}' is not checkable", item_id)),
            None => Err(format!("menu item '{}' does not exist", item_id)),
        }
    }

    pub fn menu_item_checked(&self, item_id: &str) -> Result<bool, String> {
        match self.menu_items.get(item_id) {
            Some(ManagedMenuItem::Check(item)) => Ok(item.is_checked()),
            Some(_) => Err(format!("menu item '{}' is not checkable", item_id)),
            None => Err(format!("menu item '{}' does not exist", item_id)),
        }
    }

    pub fn monitor_by_id(
        &self,
        event_loop: &EventLoopWindowTarget<Action>,
        monitor_id: u64,
    ) -> Option<MonitorHandle> {
        event_loop.available_monitors().nth(monitor_id as usize)
    }

    pub fn monitor_id(
        &self,
        event_loop: &EventLoopWindowTarget<Action>,
        monitor: &MonitorHandle,
    ) -> Option<usize> {
        event_loop
            .available_monitors()
            .position(|candidate| &candidate == monitor)
    }

    pub fn handle_tray_event_pub(&self, event: TrayIconEvent) {
        let label = event.id().as_ref().to_string();
        let (method, data) = tray_event_payload(event);
        channel::send_tray_event(&label, method, data);
    }

    pub fn handle_menu_event_pub(&self, event: MenuEvent) {
        let item_id = event.id().as_ref().to_string();
        channel::send_menu_event(&item_id, json!({ "id": item_id }));
    }

    fn build_menu_item(
        &mut self,
        parent_label: &str,
        index: usize,
        data: &Value,
    ) -> Result<ManagedMenuItem, String> {
        let item_type = data.get("type").and_then(Value::as_str).unwrap_or_else(|| {
            if data.get("items").is_some() {
                "submenu"
            } else if data.get("checked").is_some() {
                "check"
            } else {
                "normal"
            }
        });
        let item_id = data
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| self.next_menu_item_id(parent_label, index));
        let text = data.get("text").and_then(Value::as_str).unwrap_or("");
        let enabled = data.get("enabled").and_then(Value::as_bool).unwrap_or(true);

        let item = match item_type {
            "separator" => ManagedMenuItem::Predefined(PredefinedMenuItem::separator()),
            "predefined" => {
                let kind = data
                    .get("item")
                    .and_then(Value::as_str)
                    .unwrap_or("separator");
                ManagedMenuItem::Predefined(build_predefined_item(
                    kind,
                    data.get("text").and_then(Value::as_str),
                )?)
            }
            "check" => {
                let checked = data
                    .get("checked")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let item = CheckMenuItem::with_id(
                    item_id.clone(),
                    text,
                    enabled,
                    checked,
                    parse_accelerator(data)?,
                );
                self.menu_items
                    .insert(item_id.clone(), ManagedMenuItem::Check(item.clone()));
                ManagedMenuItem::Check(item)
            }
            "submenu" => {
                let submenu = Submenu::with_id(item_id.clone(), text, enabled);
                self.menus
                    .insert(item_id.clone(), ManagedMenu::Submenu(submenu.clone()));
                self.menu_items
                    .insert(item_id.clone(), ManagedMenuItem::Submenu(submenu.clone()));
                if let Some(items) = data.get("items").and_then(Value::as_array) {
                    for (child_index, child) in items.iter().enumerate() {
                        let child_item = self.build_menu_item(&item_id, child_index, child)?;
                        append_item_to_menu(&ManagedMenu::Submenu(submenu.clone()), &child_item)?;
                    }
                }
                ManagedMenuItem::Submenu(submenu)
            }
            _ => {
                let item =
                    MenuItem::with_id(item_id.clone(), text, enabled, parse_accelerator(data)?);
                self.menu_items
                    .insert(item_id.clone(), ManagedMenuItem::Normal(item.clone()));
                ManagedMenuItem::Normal(item)
            }
        };

        Ok(item)
    }

    fn next_menu_item_id(&mut self, parent_label: &str, index: usize) -> String {
        self.menu_counter += 1;
        format!("{}:{}:{}", parent_label, index, self.menu_counter)
    }
}

impl ManagedMenu {
    pub fn as_context_menu(&self) -> Result<Box<dyn ContextMenu>, String> {
        match self {
            ManagedMenu::Menu(menu) => Ok(Box::new(menu.clone())),
            ManagedMenu::Submenu(submenu) => Ok(Box::new(submenu.clone())),
        }
    }

    pub fn as_root_menu(&self) -> Result<Menu, String> {
        match self {
            ManagedMenu::Menu(menu) => Ok(menu.clone()),
            ManagedMenu::Submenu(_) => {
                Err("a submenu cannot be used as an application/window menu".to_string())
            }
        }
    }
}

fn append_item_to_menu(menu: &ManagedMenu, item: &ManagedMenuItem) -> Result<(), String> {
    match (menu, item) {
        (ManagedMenu::Menu(menu), ManagedMenuItem::Normal(item)) => append(menu, item),
        (ManagedMenu::Menu(menu), ManagedMenuItem::Check(item)) => append(menu, item),
        (ManagedMenu::Menu(menu), ManagedMenuItem::Predefined(item)) => append(menu, item),
        (ManagedMenu::Menu(menu), ManagedMenuItem::Submenu(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Normal(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Check(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Predefined(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Submenu(item)) => append(menu, item),
    }
}

fn append(menu: &impl MenuAppender, item: &dyn IsMenuItem) -> Result<(), String> {
    menu.append_item(item)
}

trait MenuAppender {
    fn append_item(&self, item: &dyn IsMenuItem) -> Result<(), String>;
}

impl MenuAppender for Menu {
    fn append_item(&self, item: &dyn IsMenuItem) -> Result<(), String> {
        self.append(item)
            .map_err(|error| format!("failed to append menu item: {}", error))
    }
}

impl MenuAppender for Submenu {
    fn append_item(&self, item: &dyn IsMenuItem) -> Result<(), String> {
        self.append(item)
            .map_err(|error| format!("failed to append submenu item: {}", error))
    }
}

fn managed_item_id(item: &ManagedMenuItem) -> String {
    match item {
        ManagedMenuItem::Normal(item) => item.id().as_ref().to_string(),
        ManagedMenuItem::Check(item) => item.id().as_ref().to_string(),
        ManagedMenuItem::Predefined(item) => item.id().as_ref().to_string(),
        ManagedMenuItem::Submenu(item) => item.id().as_ref().to_string(),
    }
}

fn parse_accelerator(data: &Value) -> Result<Option<Accelerator>, String> {
    match data.get("accelerator").and_then(Value::as_str) {
        Some(accelerator) => accelerator
            .parse()
            .map(Some)
            .map_err(|error| format!("invalid accelerator '{}': {}", accelerator, error)),
        None => Ok(None),
    }
}

fn build_predefined_item(kind: &str, text: Option<&str>) -> Result<PredefinedMenuItem, String> {
    Ok(match kind {
        "copy" => PredefinedMenuItem::copy(text),
        "cut" => PredefinedMenuItem::cut(text),
        "paste" => PredefinedMenuItem::paste(text),
        "selectAll" => PredefinedMenuItem::select_all(text),
        "undo" => PredefinedMenuItem::undo(text),
        "redo" => PredefinedMenuItem::redo(text),
        "minimize" => PredefinedMenuItem::minimize(text),
        "maximize" => PredefinedMenuItem::maximize(text),
        "fullscreen" => PredefinedMenuItem::fullscreen(text),
        "hide" => PredefinedMenuItem::hide(text),
        "hideOthers" => PredefinedMenuItem::hide_others(text),
        "showAll" => PredefinedMenuItem::show_all(text),
        "closeWindow" => PredefinedMenuItem::close_window(text),
        "quit" => PredefinedMenuItem::quit(text),
        "services" => PredefinedMenuItem::services(text),
        "bringAllToFront" => PredefinedMenuItem::bring_all_to_front(text),
        "separator" => PredefinedMenuItem::separator(),
        other => return Err(format!("unsupported predefined menu item '{}'", other)),
    })
}

/// 将 WebView 配置选项应用到 WebViewBuilder
fn apply_webview_options<'a>(
    label: String,
    mut builder: WebViewBuilder<'a>,
    data: &Value,
    rpc_state: Arc<Mutex<RpcState>>,
    protocol_state: Arc<Mutex<ProtocolState>>,
) -> Result<WebViewBuilder<'a>, String> {
    if let Some(url) = data.get("url").and_then(Value::as_str) {
        if let Some(headers) = data.get("headers") {
            builder = builder.with_url_and_headers(url, headers_from_value(headers)?);
        } else {
            builder = builder.with_url(url);
        }
    }
    if let Some(headers) = data.get("headers").filter(|_| data.get("url").is_none()) {
        builder = builder.with_headers(headers_from_value(headers)?);
    }
    if let Some(html) = data.get("html").and_then(Value::as_str) {
        builder = builder.with_html(html);
    }
    let is_transparent = data
        .get("transparent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let bg_color = color_from_value(data.get("backgroundColor").unwrap_or(&Value::Null))?;
    if let Some(color) = bg_color {
        builder = builder.with_background_color(color);
    } else if is_transparent {
        builder = builder.with_background_color((0, 0, 0, 0));
    }
    if is_transparent {
        builder = builder.with_transparent(true);
    }
    builder = builder.with_visible(true);
    builder = builder.with_autoplay(true);
    if let Some(devtools) = data.get("devtools").and_then(Value::as_bool) {
        builder = builder.with_devtools(devtools);
    }
    if let Some(user_agent) = data.get("userAgent").and_then(Value::as_str) {
        builder = builder.with_user_agent(user_agent);
    }
    builder = builder.with_hotkeys_zoom(false);
    if let Some(enabled) = data.get("clipboard").and_then(Value::as_bool) {
        builder = builder.with_clipboard(enabled);
    }
    if let Some(enabled) = data.get("acceptFirstMouse").and_then(Value::as_bool) {
        builder = builder.with_accept_first_mouse(enabled);
    }
    builder = builder.with_focused(true);
    if let Some(scripts) = data.get("initializationScripts").and_then(Value::as_array) {
        for script in scripts {
            if let Some(script) = script.as_str() {
                builder = builder.with_initialization_script(script);
            }
        }
    }

    let ipc_label = label.clone();
    let ipc_rpc_state = rpc_state.clone();
    builder = builder.with_ipc_handler(move |request| {
    let body = request.body().clone();
    let uri = request.uri().to_string();

    // 尝试解析为 RPC 消息，成功则按类型路由；失败则回退到传统 ipcMessage 透传
    if let Some(rpc_msg) = parse_ipc_message(&body) {
      match rpc_msg.msg_type {
        RpcMessageType::Request => {
          channel::send_window_event(
            &ipc_label,
            "rpcRequest",
            json!({ "rpcId": rpc_msg.id, "method": rpc_msg.method, "data": rpc_msg.data }),
          );
        }
        RpcMessageType::Response => {
          // WebView 对 Host→WebView 请求的响应：调用存储的回调
          if let Some(rpc_id) = rpc_msg.id {
            if let Ok(mut state) = ipc_rpc_state.lock() {
              if let Some(callback) = state.resolve_host_request(rpc_id) {
                if let Some(error) = &rpc_msg.error {
                  callback(Err(error.clone()));
                } else {
                  callback(Ok(rpc_msg.data));
                }
              }
            }
          }
        }
        RpcMessageType::Send => {
          channel::send_window_event(
            &ipc_label,
            "rpcMessage",
            json!({ "event": rpc_msg.event, "data": rpc_msg.data }),
          );
        }
      }
    } else {
      // 非 RPC 消息，保持向后兼容
      channel::send_window_event(
        &ipc_label,
        "ipcMessage",
        json!({
          "url": uri,
          "body": body
        }),
      );
    }
  });

    let navigation_label = label.clone();
    let navigation_allowed = data
        .get("navigationAllowed")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    builder = builder.with_navigation_handler(move |url| {
        channel::send_window_event(&navigation_label, "navigation", json!({ "url": url }));
        navigation_allowed
    });

    let new_window_label = label.clone();
    let new_window_allowed = data
        .get("newWindowAllowed")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    builder = builder.with_new_window_req_handler(move |url, _features| {
        channel::send_window_event(&new_window_label, "newWindow", json!({ "url": url }));
        if new_window_allowed {
            NewWindowResponse::Allow
        } else {
            NewWindowResponse::Deny
        }
    });

    let title_label = label.clone();
    builder = builder.with_document_title_changed_handler(move |title| {
        channel::send_window_event(
            &title_label,
            "documentTitleChanged",
            json!({ "title": title }),
        );
    });

    let load_label = label.clone();
    builder = builder.with_on_page_load_handler(move |event, url| {
        let event = match event {
            PageLoadEvent::Started => "started",
            PageLoadEvent::Finished => "finished",
        };
        channel::send_window_event(
            &load_label,
            "pageLoad",
            json!({ "event": event, "url": url }),
        );
    });

    let drag_label = label.clone();
    let prevent_default = data
        .get("dragDropPreventDefault")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    builder = builder.with_drag_drop_handler(move |event| {
        channel::send_window_event(&drag_label, "dragDrop", drag_drop_payload(event));
        prevent_default
    });

    // 注册 assets:// 和 views:// 协议 — 原样转发给 Node.js 处理
    for (scheme, event_name) in [("assets", "assetsRequest"), ("views", "protocolRequest")] {
        let event_label = label.clone();
        let state_clone = protocol_state.clone();
        builder = builder.with_asynchronous_custom_protocol(
            scheme.into(),
            move |_webview_id, request, responder| {
                let request_id = state_clone
                    .lock()
                    .expect("protocol state lock poisoned")
                    .insert(responder);

                let uri = request.uri().to_string();

                let method = request.method().to_string();
                let headers: Value = {
                    let mut map = serde_json::Map::new();
                    for (key, value) in request.headers().iter() {
                        if let Ok(v) = value.to_str() {
                            map.insert(key.as_str().to_string(), Value::String(v.to_string()));
                        }
                    }
                    Value::Object(map)
                };
                let body = request.into_body();
                let body_base64 = if body.is_empty() {
                    String::new()
                } else {
                    BASE64.encode(&body)
                };

                channel::send_window_event(
                    &event_label,
                    event_name,
                    json!({
                      "requestId": request_id,
                      "uri": uri,
                      "method": method,
                      "headers": headers,
                      "body": body_base64
                    }),
                );
            },
        );
    }

    let download_start_label = label.clone();
    let download_allowed = data
        .get("downloadAllowed")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let download_path = data
        .get("downloadPath")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    builder = builder.with_download_started_handler(move |url, path| {
        if let Some(download_path) = &download_path {
            *path = std::path::PathBuf::from(download_path);
        }
        channel::send_window_event(
            &download_start_label,
            "downloadStarted",
            json!({ "url": url, "path": path.to_string_lossy() }),
        );
        download_allowed
    });

    let download_done_label = label;
    builder = builder.with_download_completed_handler(move |url, path, success| {
        channel::send_window_event(
            &download_done_label,
            "downloadCompleted",
            json!({
              "url": url,
              "path": path.map(|p| p.to_string_lossy().to_string()),
              "success": success
            }),
        );
    });

    Ok(builder)
}

pub fn headers_from_value(value: &Value) -> Result<wry::http::HeaderMap, String> {
    let mut headers = wry::http::HeaderMap::new();
    if value.is_null() {
        return Ok(headers);
    }
    let object = value
        .as_object()
        .ok_or_else(|| "headers must be an object".to_string())?;
    for (key, value) in object {
        let value = value
            .as_str()
            .ok_or_else(|| format!("header '{}' must be a string", key))?;
        let name = wry::http::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|error| format!("invalid header name '{}': {}", key, error))?;
        let value = wry::http::HeaderValue::from_str(value)
            .map_err(|error| format!("invalid header value '{}': {}", key, error))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn color_from_value(value: &Value) -> Result<Option<(u8, u8, u8, u8)>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let array = value
        .as_array()
        .ok_or_else(|| "backgroundColor must be [r, g, b, a]".to_string())?;
    if array.len() != 4 {
        return Err("backgroundColor must contain exactly 4 values".to_string());
    }
    let mut values = [0u8; 4];
    for (index, item) in array.iter().enumerate() {
        let value = item
            .as_u64()
            .ok_or_else(|| "backgroundColor values must be integers".to_string())?;
        if value > u8::MAX as u64 {
            return Err("backgroundColor values must be between 0 and 255".to_string());
        }
        values[index] = value as u8;
    }
    Ok(Some((values[0], values[1], values[2], values[3])))
}

fn drag_drop_payload(event: DragDropEvent) -> Value {
    match event {
        DragDropEvent::Enter { paths, position } => json!({
          "event": "enter",
          "paths": paths_to_json(paths),
          "position": { "x": position.0, "y": position.1 }
        }),
        DragDropEvent::Over { position } => json!({
          "event": "over",
          "position": { "x": position.0, "y": position.1 }
        }),
        DragDropEvent::Drop { paths, position } => json!({
          "event": "drop",
          "paths": paths_to_json(paths),
          "position": { "x": position.0, "y": position.1 }
        }),
        DragDropEvent::Leave => json!({ "event": "leave" }),
        _ => json!({ "event": "unknown" }),
    }
}

fn paths_to_json(paths: Vec<std::path::PathBuf>) -> Value {
    Value::Array(
        paths
            .into_iter()
            .map(|path| Value::String(path.to_string_lossy().to_string()))
            .collect(),
    )
}

fn tray_event_payload(event: TrayIconEvent) -> (&'static str, Value) {
    match event {
        TrayIconEvent::Click {
            position,
            rect,
            button,
            button_state,
            ..
        } => (
            "click",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect),
              "button": format!("{:?}", button),
              "state": format!("{:?}", button_state)
            }),
        ),
        TrayIconEvent::DoubleClick {
            position,
            rect,
            button,
            ..
        } => (
            "doubleClick",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect),
              "button": format!("{:?}", button)
            }),
        ),
        TrayIconEvent::Enter { position, rect, .. } => (
            "enter",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect)
            }),
        ),
        TrayIconEvent::Move { position, rect, .. } => (
            "move",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect)
            }),
        ),
        TrayIconEvent::Leave { position, rect, .. } => (
            "leave",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect)
            }),
        ),
        _ => ("unknown", Value::Null),
    }
}

fn tray_rect_payload(rect: tray_icon::Rect) -> Value {
    json!({
      "x": rect.position.x,
      "y": rect.position.y,
      "width": rect.size.width,
      "height": rect.size.height
    })
}

/// 构建 tao 窗口构建器，根据配置设置窗口属性
pub fn build_window_builder(
    app: &Application,
    event_loop: &EventLoopWindowTarget<Action>,
    data: &Value,
) -> Result<WindowBuilder, String> {
    let mut builder = WindowBuilder::new();
    let width = data.get("width").and_then(Value::as_f64);
    let height = data.get("height").and_then(Value::as_f64);
    if width.is_some() || height.is_some() {
        builder = builder.with_inner_size(LogicalSize::new(
            width.unwrap_or(800.0),
            height.unwrap_or(600.0),
        ));
    }
    let min_width = data.get("minWidth").and_then(Value::as_f64);
    let min_height = data.get("minHeight").and_then(Value::as_f64);
    if min_width.is_some() || min_height.is_some() {
        builder = builder.with_min_inner_size(LogicalSize::new(
            min_width.unwrap_or(0.0),
            min_height.unwrap_or(0.0),
        ));
    }
    let max_width = data.get("maxWidth").and_then(Value::as_f64);
    let max_height = data.get("maxHeight").and_then(Value::as_f64);
    if max_width.is_some() || max_height.is_some() {
        builder = builder.with_max_inner_size(LogicalSize::new(
            max_width.unwrap_or(f64::MAX),
            max_height.unwrap_or(f64::MAX),
        ));
    }
    let x = data.get("x").and_then(Value::as_f64);
    let y = data.get("y").and_then(Value::as_f64);
    if let (Some(x), Some(y)) = (x, y) {
        builder = builder.with_position(LogicalPosition::new(x, y));
    }
    if let Some(resizable) = data.get("resizable").and_then(Value::as_bool) {
        builder = builder.with_resizable(resizable);
    }
    if let Some(minimizable) = data.get("minimizable").and_then(Value::as_bool) {
        builder = builder.with_minimizable(minimizable);
    }
    if let Some(maximizable) = data.get("maximizable").and_then(Value::as_bool) {
        builder = builder.with_maximizable(maximizable);
    }
    if let Some(closable) = data.get("closable").and_then(Value::as_bool) {
        builder = builder.with_closable(closable);
    }
    if let Some(buttons) = data.get("enabledButtons").and_then(Value::as_array) {
        let names = buttons
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<&str>>();
        builder = builder
            .with_closable(names.contains(&"close"))
            .with_minimizable(names.contains(&"minimize"))
            .with_maximizable(names.contains(&"maximize"));
    }
    if let Some(title) = data.get("title").and_then(Value::as_str) {
        builder = builder.with_title(title);
    }
    if let Some(maximized) = data.get("maximized").and_then(Value::as_bool) {
        builder = builder.with_maximized(maximized);
    }
    if let Some(visible) = data.get("visible").and_then(Value::as_bool) {
        builder = builder.with_visible(visible);
    }
    let is_transparent = data
        .get("transparent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if is_transparent {
        builder = builder.with_transparent(true);
    }
    if let Some(borderless) = data.get("borderless").and_then(Value::as_bool) {
        builder = builder.with_decorations(!borderless);
    }
    if let Some(decorations) = data.get("decorations").and_then(Value::as_bool) {
        builder = builder.with_decorations(decorations);
    }
    if let Some(always_on_top) = data.get("alwaysOnTop").and_then(Value::as_bool) {
        builder = builder.with_always_on_top(always_on_top);
    }
    if let Some(always_on_bottom) = data.get("alwaysOnBottom").and_then(Value::as_bool) {
        builder = builder.with_always_on_bottom(always_on_bottom);
    }
    if let Some(icon_path) = data.get("windowIcon").and_then(Value::as_str) {
        builder = builder.with_window_icon(Some(load_tao_icon(icon_path)?));
    }
    let win_bg_color = color_from_value(data.get("windowBackgroundColor").unwrap_or(&Value::Null))?;
    if let Some(color) = win_bg_color {
        builder = builder.with_background_color(color);
    } else if is_transparent {
        builder = builder.with_background_color((0, 0, 0, 0));
    }
    if let Some(theme) = data.get("theme").and_then(Value::as_str) {
        builder = builder.with_theme(theme_from_str(theme));
    }
    if let Some(focused) = data
        .get("focused")
        .or_else(|| data.get("active"))
        .and_then(Value::as_bool)
    {
        builder = builder.with_focused(focused);
    }
    if let Some(protected) = data.get("contentProtected").and_then(Value::as_bool) {
        builder = builder.with_content_protection(protected);
    }
    if let Some(visible) = data.get("visibleOnAllWorkspaces").and_then(Value::as_bool) {
        builder = builder.with_visible_on_all_workspaces(visible);
    }
    if let Some(fullscreen) = data.get("fullscreen") {
        if let Some(fullscreen) = fullscreen_from_value(app, event_loop, fullscreen) {
            builder = builder.with_fullscreen(Some(fullscreen));
        }
    }
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::WindowBuilderExtMacOS;
        if let Some(style) = data.get("titleBarStyle").and_then(Value::as_str) {
            match style {
                "hidden" => {
                    builder = builder.with_titlebar_transparent(true);
                }
                "hiddenInset" => {
                    builder = builder.with_titlebar_transparent(true);
                    builder = builder.with_fullsize_content_view(true);
                }
                _ => {}
            }
        }
        if let Some(position) = data.get("trafficLightPosition") {
            if let Some(pos) = position_from_value(position) {
                builder =
                    builder.with_traffic_light_inset(tao::dpi::LogicalPosition::new(pos.x, pos.y));
            }
        }
    }
    Ok(builder)
}

pub fn size_from_value(value: &Value) -> Option<Size> {
    let object = value.as_object()?;
    let width = object.get("width")?.as_f64()?;
    let height = object.get("height")?.as_f64()?;
    Some(Size::Logical(LogicalSize::new(width, height)))
}

pub fn position_from_value(value: &Value) -> Option<LogicalPosition<f64>> {
    let object = value.as_object()?;
    let x = object.get("x")?.as_f64()?;
    let y = object.get("y")?.as_f64()?;
    Some(LogicalPosition::new(x, y))
}

pub fn theme_from_str(theme: &str) -> Option<Theme> {
    match theme {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

pub fn fullscreen_from_value(
    app: &Application,
    event_loop: &EventLoopWindowTarget<Action>,
    value: &Value,
) -> Option<Fullscreen> {
    if value.as_bool() == Some(false) || value.is_null() {
        return None;
    }
    if let Some(monitor_id) = value.as_u64() {
        return Some(Fullscreen::Borderless(
            app.monitor_by_id(event_loop, monitor_id),
        ));
    }
    if value.as_bool() == Some(true) {
        return Some(Fullscreen::Borderless(None));
    }
    None
}
