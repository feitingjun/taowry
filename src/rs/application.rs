//! Application 核心模块
//!
//! 包含 Application 结构体和事件循环实现，
//! 管理窗口、菜单、托盘的生命周期。

use crate::channel;
use crate::protocol::ProtocolState;
use crate::rpc::RpcState;
use crate::window::{load_tray_icon_base64, load_tray_icon_from_bytes, BrowserWindow};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::monitor::MonitorHandle;
use tao::window::{WindowBuilder, WindowId};
use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use wry::{Rect, WebViewBuilder};

/// 事件循环中的用户事件类型
pub enum Action {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}

/// 被管理的菜单类型（顶层菜单或子菜单）
pub use crate::menu_manager::{ManagedMenu, ManagedMenuItem, append_item_to_menu, managed_item_id, parse_accelerator, build_predefined_item};
pub(crate) use crate::window_builder::{apply_webview_options, headers_from_value, build_window_builder, size_from_value, position_from_value, theme_from_str, fullscreen_from_value};
pub(crate) use crate::tray_events::tray_event_payload;


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
        // 先清理 RPC 状态和协议状态，防止回调泄漏
        if let Some(window) = self.windows.get(label) {
            if let Ok(mut rpc) = window.rpc_state.lock() {
                rpc.clear();
            }
            if let Ok(mut protocol) = window.protocol_state.lock() {
                protocol.clear();
            }
        }
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
        if let Some(icon_base64) = data.get("icon").and_then(Value::as_str) {
            builder = builder.with_icon(load_tray_icon_base64(icon_base64)?);
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

    pub fn set_tray_icon_bytes(&self, label: &str, icon_bytes: &[u8]) -> Result<(), String> {
        let tray = self
            .trays
            .get(label)
            .ok_or_else(|| format!("tray '{}' does not exist", label))?;
        let icon = load_tray_icon_from_bytes(icon_bytes)?;
        tray.set_icon(Some(icon))
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

