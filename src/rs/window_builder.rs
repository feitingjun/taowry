//! 窗口与 WebView 构建模块
//!
//! 提供窗口构建器和 WebView 构建器的配置函数，
//! 以及颜色解析、响应头构建、拖拽事件转换等辅助工具。

use std::sync::{Arc, Mutex};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use tao::dpi::{LogicalPosition, LogicalSize, Size};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Fullscreen, Theme, WindowBuilder};
use wry::{DragDropEvent, NewWindowResponse, PageLoadEvent, WebViewBuilder};

use crate::application::{Action, Application};
use crate::channel;
use crate::protocol::ProtocolState;
use crate::rpc::{parse_ipc_message, parse_win_command, RpcMessageType, RpcState, WinCommandQueue};
use crate::window::load_tao_icon_base64;


/// 将 WebView 配置选项应用到 WebViewBuilder
pub fn apply_webview_options<'a>(
    label: String,
    mut builder: WebViewBuilder<'a>,
    data: &Value,
    rpc_state: Arc<Mutex<RpcState>>,
    protocol_state: Arc<Mutex<ProtocolState>>,
    win_cmd_queue: WinCommandQueue,
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
    let ipc_cmd_queue = win_cmd_queue.clone();
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
        RpcMessageType::WinControl => {
          // 前端直接窗口控制：解析命令并推入队列，不转发给 Node.js
          if let Some(method) = &rpc_msg.method {
            if let Some(cmd) = parse_win_command(method, &rpc_msg.data, rpc_msg.id) {
              if let Ok(mut queue) = ipc_cmd_queue.lock() {
                queue.push(cmd);
              }
            }
          }
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

pub fn color_from_value(value: &Value) -> Result<Option<(u8, u8, u8, u8)>, String> {
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

pub fn drag_drop_payload(event: DragDropEvent) -> Value {
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

pub fn paths_to_json(paths: Vec<std::path::PathBuf>) -> Value {
    Value::Array(
        paths
            .into_iter()
            .map(|path| Value::String(path.to_string_lossy().to_string()))
            .collect(),
    )
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
    if let Some(icon_base64) = data.get("windowIcon").and_then(Value::as_str) {
        builder = builder.with_window_icon(Some(load_tao_icon_base64(icon_base64)?));
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
