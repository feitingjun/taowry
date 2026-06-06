//! IPC 消息处理模块
//!
//! 解析来自 Node.js 的 IPC 消息并分发给对应的处理方法，
//! 处理窗口操作、菜单管理、托盘管理等命令，并将结果返回给 Node.js。

use crate::application::{
  build_window_builder, fullscreen_from_value, headers_from_value, position_from_value,
  size_from_value, theme_from_str, Action, Application,
};
use serde_json::{json, Value};
use std::io::{self, Write};
use tao::dpi::Size;
use tao::event_loop::{ControlFlow, EventLoopWindowTarget};
use tao::window::{
  CursorIcon, Fullscreen, ProgressBarState, ProgressState, ResizeDirection, UserAttentionType,
  WindowSizeConstraints,
};
use wry::webview_version;

/// IPC 消息前缀
pub const IO_CHANNEL_PREFIX: &str = "_ioc:";

/// 发送 JSON 格式的 IPC 消息到 stdout
pub fn send_io_message(msg: Value) {
  match serde_json::to_string(&msg) {
    Ok(json_str) => {
      let mut output = io::stdout();
      let _ = writeln!(output, "{}{}", IO_CHANNEL_PREFIX, json_str);
      let _ = output.flush();
    }
    Err(error) => {
      eprintln!("failed to serialize ipc message: {}", error);
    }
  }
}

/// 发送窗口事件
pub fn send_window_event(label: &str, method: &str, data: Value) {
  send_io_message(json!({
    "type": "windowEvent",
    "label": label,
    "method": method,
    "data": data
  }));
}

/// 发送应用事件
pub fn send_app_event(method: &str, data: Value) {
  send_io_message(json!({
    "type": "appEvent",
    "label": "app",
    "method": method,
    "data": data
  }));
}

/// 处理来自 Node.js 的 IPC 消息
pub fn handle_listen(
  app: &mut Application,
  raw: &str,
  event_loop: &EventLoopWindowTarget<Action>,
  control_flow: &mut ControlFlow,
) {
  let message = match serde_json::from_str::<Value>(raw) {
    Ok(value) => value,
    Err(error) => {
      send_error("", "", "", format!("invalid ipc json: {}", error));
      return;
    }
  };
  let id = message.get("id").and_then(Value::as_str).unwrap_or("");
  let label = message.get("label").and_then(Value::as_str).unwrap_or("");
  let method = message.get("method").and_then(Value::as_str).unwrap_or("");
  let data = message.get("data").unwrap_or(&Value::Null);

  if method == "evaluate_script_with_callback" {
    handle_evaluate_script_with_callback(app, id, label, method, data);
    return;
  }

  let result = handle_method(app, event_loop, control_flow, label, method, data);
  match result {
    Ok(data) => send_response(id, label, method, data),
    Err(error) => send_error(id, label, method, error),
  }
}

fn handle_evaluate_script_with_callback(
  app: &Application,
  id: &str,
  label: &str,
  method: &str,
  data: &Value,
) {
  let result = app
    .get_window(label)
    .ok_or_else(|| format!("window '{}' does not exist", label))
    .and_then(|window| {
      let script = required_string(data)?;
      let id = id.to_string();
      let label = label.to_string();
      let method = method.to_string();
      window
        .evaluate_script_with_callback(&script, move |result| {
          send_response(&id, &label, &method, Value::String(result));
        })
        .map_err(|error| error.to_string())
    });

  if let Err(error) = result {
    send_error(id, label, method, error);
  }
}

fn handle_method(
  app: &mut Application,
  event_loop: &EventLoopWindowTarget<Action>,
  control_flow: &mut ControlFlow,
  label: &str,
  method: &str,
  data: &Value,
) -> Result<Value, String> {
  match method {
    "app_quit" | "quit" => {
      *control_flow = ControlFlow::Exit;
      Ok(Value::Null)
    }
    "app_window_labels" => Ok(Value::Array(
      app
        .windows
        .keys()
        .cloned()
        .map(Value::String)
        .collect::<Vec<Value>>(),
    )),
    "webview_version" => webview_version()
      .map(Value::String)
      .map_err(|error| error.to_string()),
    "create" | "create_window" => {
      let builder = build_window_builder(app, event_loop, data)?;
      let id = app.create_new_window(event_loop, label.to_string(), builder, data)?;
      Ok(Value::String(id))
    }
    "create_menu" => {
      app.create_menu(label.to_string(), data)?;
      Ok(Value::Null)
    }
    "append_menu_item" => {
      let menu_label = string_field(data, "menu").unwrap_or_else(|| label.to_string());
      let item = data.get("item").unwrap_or(data);
      Ok(Value::String(app.append_menu_item(&menu_label, item)?))
    }
    "set_application_menu" => {
      let menu_label = optional_string(data).or_else(|| string_field(data, "menu")).unwrap_or_else(|| label.to_string());
      app.set_application_menu(&menu_label)?;
      Ok(Value::Null)
    }
    "set_window_menu" => {
      let window_label = string_field(data, "window").unwrap_or_else(|| label.to_string());
      let menu_label = string_field(data, "menu").ok_or_else(|| "menu is required".to_string())?;
      app.set_window_menu(&window_label, &menu_label)?;
      Ok(Value::Null)
    }
    "set_menu_item_enabled" => {
      let id = string_field(data, "id").unwrap_or_else(|| label.to_string());
      app.set_menu_item_enabled(&id, bool_field(data, "enabled")?)?;
      Ok(Value::Null)
    }
    "set_menu_item_text" => {
      let id = string_field(data, "id").unwrap_or_else(|| label.to_string());
      let text = string_field(data, "text").ok_or_else(|| "text is required".to_string())?;
      app.set_menu_item_text(&id, &text)?;
      Ok(Value::Null)
    }
    "set_menu_item_checked" => {
      let id = string_field(data, "id").unwrap_or_else(|| label.to_string());
      app.set_menu_item_checked(&id, bool_field(data, "checked")?)?;
      Ok(Value::Null)
    }
    "is_menu_item_checked" => {
      let id = optional_string(data).or_else(|| string_field(data, "id")).unwrap_or_else(|| label.to_string());
      Ok(Value::Bool(app.menu_item_checked(&id)?))
    }
    "create_tray" => {
      app.create_tray(label.to_string(), data)?;
      Ok(Value::Null)
    }
    "remove_tray" => {
      app.remove_tray(label)?;
      Ok(Value::Null)
    }
    "set_tray_icon" => {
      app.set_tray_icon(label, optional_string(data).as_deref())?;
      Ok(Value::Null)
    }
    "set_tray_menu" => {
      app.set_tray_menu(label, optional_string(data).or_else(|| string_field(data, "menu")).as_deref())?;
      Ok(Value::Null)
    }
    "set_tray_tooltip" => {
      let tray = app
        .trays
        .get(label)
        .ok_or_else(|| format!("tray '{}' does not exist", label))?;
      tray
        .set_tooltip(optional_string(data).as_deref())
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "set_tray_title" => {
      let tray = app
        .trays
        .get(label)
        .ok_or_else(|| format!("tray '{}' does not exist", label))?;
      tray.set_title(optional_string(data).as_deref());
      Ok(Value::Null)
    }
    "set_tray_visible" => {
      let tray = app
        .trays
        .get(label)
        .ok_or_else(|| format!("tray '{}' does not exist", label))?;
      tray
        .set_visible(bool_value(data)?)
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "tray_rect" => {
      let tray = app
        .trays
        .get(label)
        .ok_or_else(|| format!("tray '{}' does not exist", label))?;
      Ok(match tray.rect() {
        Some(rect) => json!({
          "x": rect.position.x,
          "y": rect.position.y,
          "width": rect.size.width,
          "height": rect.size.height
        }),
        None => Value::Null,
      })
    }
    // ===== Dock (macOS) =====
    "show_dock_icon" => {
      #[cfg(target_os = "macos")]
      {
        crate::dock::platform::set_dock_visible(true);
        Ok(Value::Null)
      }
      #[cfg(not(target_os = "macos"))]
      Err("dock operations are only supported on macOS".to_string())
    }
    "hide_dock_icon" => {
      #[cfg(target_os = "macos")]
      {
        crate::dock::platform::set_dock_visible(false);
        Ok(Value::Null)
      }
      #[cfg(not(target_os = "macos"))]
      Err("dock operations are only supported on macOS".to_string())
    }
    "set_dock_badge" => {
      #[cfg(target_os = "macos")]
      {
        let text = optional_string(data).unwrap_or_default();
        crate::dock::platform::set_dock_badge(&text);
        Ok(Value::Null)
      }
      #[cfg(not(target_os = "macos"))]
      Err("dock operations are only supported on macOS".to_string())
    }
    "bounce_dock" => {
      #[cfg(target_os = "macos")]
      {
        crate::dock::platform::bounce_dock();
        Ok(Value::Null)
      }
      #[cfg(not(target_os = "macos"))]
      Err("dock operations are only supported on macOS".to_string())
    }
    "set_dock_menu" => {
      #[cfg(target_os = "macos")]
      {
        let menu_label = optional_string(data)
          .or_else(|| string_field(data, "menu"))
          .ok_or_else(|| "menu label is required".to_string())?;
        app.set_dock_menu(&menu_label)?;
        Ok(Value::Null)
      }
      #[cfg(not(target_os = "macos"))]
      Err("dock operations are only supported on macOS".to_string())
    }
    // ===== 显示器 =====
    "primary_monitor" => Ok(event_loop
      .primary_monitor()
      .and_then(|monitor| monitor_info(app, event_loop, monitor))
      .unwrap_or(Value::Null)),
    "get_monitor_list" | "monitors" => Ok(Value::Array(
      event_loop
        .available_monitors()
        .filter_map(|monitor| monitor_info(app, event_loop, monitor))
        .collect(),
    )),
    "monitor_from_point" => {
      let object = data.as_object().ok_or_else(|| "point is required".to_string())?;
      let x = object.get("x").and_then(Value::as_f64).ok_or_else(|| "x is required".to_string())?;
      let y = object.get("y").and_then(Value::as_f64).ok_or_else(|| "y is required".to_string())?;
      Ok(event_loop
        .available_monitors()
        .find(|monitor| {
          let scale = monitor.scale_factor();
          let px = (x * scale) as i32;
          let py = (y * scale) as i32;
          let pos = monitor.position();
          let size = monitor.size();
          px >= pos.x && px < pos.x + size.width as i32
            && py >= pos.y && py < pos.y + size.height as i32
        })
        .and_then(|monitor| monitor_info(app, event_loop, monitor))
        .unwrap_or(Value::Null))
    }
    _ => handle_window_method(app, event_loop, label, method, data),
  }
}

fn handle_window_method(
  app: &mut Application,
  event_loop: &EventLoopWindowTarget<Action>,
  label: &str,
  method: &str,
  data: &Value,
) -> Result<Value, String> {
  let window = app
    .get_window(label)
    .ok_or_else(|| format!("window '{}' does not exist", label))?;

  match method {
    "id" => Ok(Value::String(window.id_string())),
    "close" => {
      app.close_window(label);
      Ok(Value::Null)
    }
    "request_redraw" => {
      window.request_redraw();
      Ok(Value::Null)
    }
    "set_url" => {
      window.set_url(required_string(data)?.as_str()).map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "load_url_with_headers" => {
      let url = string_field(data, "url").ok_or_else(|| "url is required".to_string())?;
      let headers = headers_from_value(data.get("headers").unwrap_or(&Value::Null))?;
      window.load_url_with_headers(&url, headers).map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "url" => window.url().map(Value::String).map_err(|error| error.to_string()),
    "evaluate_script" => {
      window
        .evaluate_script(required_string(data)?.as_str())
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "print" => {
      window.print().map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "open_devtools" => {
      window.open_devtools();
      Ok(Value::Null)
    }
    "close_devtools" => {
      window.close_devtools();
      Ok(Value::Null)
    }
    "is_devtools_open" => Ok(Value::Bool(window.is_devtools_open())),
    "zoom" => {
      window.zoom(number_value(data)?).map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "scale_factor" => Ok(number_json(window.scale_factor())),
    "clear_all_browsing_data" => {
      window.clear_all_browsing_data().map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "set_background_color" => {
      window
        .set_background_color(color_value(data)?)
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "set_window_background_color" => {
      let color = if data.is_null() { None } else { Some(color_value(data)?) };
      window.set_window_background_color(color);
      Ok(Value::Null)
    }
    "inner_position" => window
      .inner_position()
      .map(|position| json!({ "x": position.x, "y": position.y }))
      .map_err(|error| error.to_string()),
    "outer_position" => window
      .outer_position()
      .map(|position| json!({ "x": position.x, "y": position.y }))
      .map_err(|error| error.to_string()),
    "set_outer_position" => {
      window.set_outer_position(position_from_value(data).ok_or_else(|| "position is invalid".to_string())?);
      Ok(Value::Null)
    }
    "inner_size" => Ok(size_to_value(window.inner_size())),
    "set_inner_size" => Ok(size_to_value(window.set_inner_size(size_value(data)?))),
    "outer_size" => Ok(size_to_value(window.outer_size())),
    "set_min_inner_size" => {
      if data.is_null() {
        window.set_min_inner_size::<Size>(None);
      } else {
        window.set_min_inner_size(Some(size_value(data)?));
      }
      Ok(Value::Null)
    }
    "set_max_inner_size" => {
      if data.is_null() {
        window.set_max_inner_size::<Size>(None);
      } else {
        window.set_max_inner_size(Some(size_value(data)?));
      }
      Ok(Value::Null)
    }
    "set_inner_size_constraints" => {
      window.set_inner_size_constraints(window_constraints_value(data)?);
      Ok(Value::Null)
    }
    "set_title" => {
      window.set_title(&required_string(data)?);
      Ok(Value::Null)
    }
    "title" => Ok(Value::String(window.title())),
    "set_visible" => {
      window.set_visible(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_visible" => Ok(Value::Bool(window.is_visible())),
    "focus_window" => {
      window.focus_window();
      Ok(Value::Null)
    }
    "has_focus" => Ok(Value::Bool(window.has_focus())),
    "set_resizable" => {
      window.set_resizable(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_resizable" => Ok(Value::Bool(window.is_resizable())),
    "set_minimizable" => {
      window.set_minimizable(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_minimizable" => Ok(Value::Bool(window.is_minimizable())),
    "set_maximizable" => {
      window.set_maximizable(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_maximizable" => Ok(Value::Bool(window.is_maximizable())),
    "set_closable" => {
      window.set_closable(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_closable" => Ok(Value::Bool(window.is_closable())),
    "set_enabled_buttons" => {
      let (close, minimize, maximize) = enabled_buttons_value(data)?;
      window.set_enabled_buttons(close, minimize, maximize);
      Ok(Value::Null)
    }
    "enabled_buttons" => Ok(Value::Array(
      window
        .enabled_buttons()
        .into_iter()
        .map(|button| Value::String(button.to_string()))
        .collect(),
    )),
    "set_minimized" => {
      window.set_minimized(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_minimized" => Ok(Value::Bool(window.is_minimized())),
    "set_maximized" => {
      window.set_maximized(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_maximized" => Ok(Value::Bool(window.is_maximized())),
    "fullscreen" => {
      window.set_fullscreen(Some(
        fullscreen_from_value(app, event_loop, data).unwrap_or(Fullscreen::Borderless(None)),
      ));
      Ok(Value::Null)
    }
    "unfullscreen" => {
      window.set_fullscreen(None);
      Ok(Value::Null)
    }
    "is_fullscreen" => match window.fullscreen() {
      Some(Fullscreen::Borderless(Some(monitor))) => {
        Ok(app.monitor_id(event_loop, &monitor).map(|id| json!(id)).unwrap_or(Value::Bool(true)))
      }
      Some(Fullscreen::Borderless(None)) | Some(Fullscreen::Exclusive(_)) => Ok(Value::Bool(true)),
      Some(_) => Ok(Value::Bool(true)),
      None => Ok(Value::Bool(false)),
    },
    "set_decorations" => {
      window.set_decorations(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_decorated" => Ok(Value::Bool(window.is_decorated())),
    "set_always_on_top" => {
      window.set_always_on_top(bool_value(data)?);
      Ok(Value::Null)
    }
    "is_always_on_top" => Ok(Value::Bool(window.is_always_on_top())),
    "set_always_on_bottom" => {
      window.set_always_on_bottom(bool_value(data)?);
      Ok(Value::Null)
    }
    "set_window_icon" => {
      window.set_window_icon(&required_string(data)?)?;
      Ok(Value::Null)
    }
    "set_ime_position" => {
      window.set_ime_position(position_from_value(data).ok_or_else(|| "position is invalid".to_string())?);
      Ok(Value::Null)
    }
    "set_progress_bar" => {
      window.set_progress_bar(progress_value(data)?);
      Ok(Value::Null)
    }
    "request_user_attention" => {
      window.request_user_attention(user_attention_value(data)?);
      Ok(Value::Null)
    }
    "set_theme" => {
      window.set_theme(optional_string(data).and_then(|theme| theme_from_str(&theme)));
      Ok(Value::Null)
    }
    "theme" => Ok(Value::String(match window.theme() {
      tao::window::Theme::Light => "light".to_string(),
      tao::window::Theme::Dark => "dark".to_string(),
      _ => "light".to_string(),
    })),
    "set_content_protection" => {
      window.set_content_protection(bool_value(data)?);
      Ok(Value::Null)
    }
    "set_visible_on_all_workspaces" => {
      window.set_visible_on_all_workspaces(bool_value(data)?);
      Ok(Value::Null)
    }
    "set_cursor_icon" => {
      window.set_cursor_icon(cursor_icon_value(&required_string(data)?)?);
      Ok(Value::Null)
    }
    "set_cursor_position" => {
      window
        .set_cursor_position(position_from_value(data).ok_or_else(|| "position is invalid".to_string())?)
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "set_cursor_grab" => {
      window.set_cursor_grab(bool_value(data)?).map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "set_cursor_visible" => {
      window.set_cursor_visible(bool_value(data)?);
      Ok(Value::Null)
    }
    "drag_window" => {
      window.drag_window().map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "drag_resize_window" => {
      window
        .drag_resize_window(resize_direction_value(&required_string(data)?)?)
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "set_ignore_cursor_events" => {
      window
        .set_ignore_cursor_events(bool_value(data)?)
        .map_err(|error| error.to_string())?;
      Ok(Value::Null)
    }
    "cursor_position" => window
      .cursor_position()
      .map(|position| json!({ "x": position.x, "y": position.y }))
      .map_err(|error| error.to_string()),
    _ => Err(format!("method '{}' does not exist", method)),
  }
}

fn send_response(id: &str, label: &str, method: &str, data: Value) {
  send_io_message(json!({
    "id": id,
    "label": label,
    "method": method,
    "type": "response",
    "data": data
  }));
}

fn send_error(id: &str, label: &str, method: &str, error: String) {
  send_io_message(json!({
    "id": id,
    "label": label,
    "method": method,
    "type": "response",
    "error": error
  }));
}

fn monitor_info(
  app: &Application,
  event_loop: &EventLoopWindowTarget<Action>,
  monitor: tao::monitor::MonitorHandle,
) -> Option<Value> {
  let id = app.monitor_id(event_loop, &monitor)?;
  let size = monitor.size();
  let position = monitor.position();
  Some(json!({
    "monitorId": id,
    "name": monitor.name(),
    "width": size.width,
    "height": size.height,
    "x": position.x,
    "y": position.y,
    "scaleFactor": monitor.scale_factor()
  }))
}

fn optional_string(value: &Value) -> Option<String> {
  value.as_str().map(ToString::to_string)
}

fn required_string(value: &Value) -> Result<String, String> {
  optional_string(value).ok_or_else(|| "expected string".to_string())
}

fn string_field(value: &Value, key: &str) -> Option<String> {
  value.get(key).and_then(Value::as_str).map(ToString::to_string)
}

fn bool_value(value: &Value) -> Result<bool, String> {
  value.as_bool().ok_or_else(|| "expected boolean".to_string())
}

fn bool_field(value: &Value, key: &str) -> Result<bool, String> {
  value
    .get(key)
    .and_then(Value::as_bool)
    .ok_or_else(|| format!("{} is required", key))
}

fn number_value(value: &Value) -> Result<f64, String> {
  value.as_f64().ok_or_else(|| "expected number".to_string())
}

fn number_json(value: f64) -> Value {
  serde_json::Number::from_f64(value)
    .map(Value::Number)
    .unwrap_or(Value::Null)
}

fn size_value(value: &Value) -> Result<Size, String> {
  size_from_value(value).ok_or_else(|| "size is invalid".to_string())
}

fn size_to_value(size: tao::dpi::PhysicalSize<u32>) -> Value {
  json!({ "width": size.width, "height": size.height })
}

fn color_value(value: &Value) -> Result<(u8, u8, u8, u8), String> {
  let array = value
    .as_array()
    .ok_or_else(|| "color must be [r, g, b, a]".to_string())?;
  if array.len() != 4 {
    return Err("color must contain exactly 4 values".to_string());
  }
  let mut values = [0u8; 4];
  for (index, item) in array.iter().enumerate() {
    let value = item
      .as_u64()
      .ok_or_else(|| "color values must be integers".to_string())?;
    if value > u8::MAX as u64 {
      return Err("color values must be between 0 and 255".to_string());
    }
    values[index] = value as u8;
  }
  Ok((values[0], values[1], values[2], values[3]))
}

fn enabled_buttons_value(value: &Value) -> Result<(bool, bool, bool), String> {
  let buttons = value
    .as_array()
    .ok_or_else(|| "enabledButtons must be an array".to_string())?
    .iter()
    .filter_map(Value::as_str)
    .collect::<Vec<&str>>();
  Ok((
    buttons.contains(&"close"),
    buttons.contains(&"minimize"),
    buttons.contains(&"maximize"),
  ))
}

fn user_attention_value(value: &Value) -> Result<Option<UserAttentionType>, String> {
  if value.is_null() {
    return Ok(None);
  }
  Ok(Some(match required_string(value)?.as_str() {
    "critical" => UserAttentionType::Critical,
    "informational" => UserAttentionType::Informational,
    other => return Err(format!("invalid user attention type '{}'", other)),
  }))
}

fn resize_direction_value(value: &str) -> Result<ResizeDirection, String> {
  Ok(match value {
    "east" => ResizeDirection::East,
    "north" => ResizeDirection::North,
    "northEast" => ResizeDirection::NorthEast,
    "northWest" => ResizeDirection::NorthWest,
    "south" => ResizeDirection::South,
    "southEast" => ResizeDirection::SouthEast,
    "southWest" => ResizeDirection::SouthWest,
    "west" => ResizeDirection::West,
    other => return Err(format!("invalid resize direction '{}'", other)),
  })
}

fn cursor_icon_value(value: &str) -> Result<CursorIcon, String> {
  Ok(match value {
    "default" => CursorIcon::Default,
    "crosshair" => CursorIcon::Crosshair,
    "hand" => CursorIcon::Hand,
    "arrow" => CursorIcon::Arrow,
    "move" => CursorIcon::Move,
    "text" => CursorIcon::Text,
    "wait" => CursorIcon::Wait,
    "help" => CursorIcon::Help,
    "progress" => CursorIcon::Progress,
    "notAllowed" => CursorIcon::NotAllowed,
    "contextMenu" => CursorIcon::ContextMenu,
    "cell" => CursorIcon::Cell,
    "verticalText" => CursorIcon::VerticalText,
    "alias" => CursorIcon::Alias,
    "copy" => CursorIcon::Copy,
    "noDrop" => CursorIcon::NoDrop,
    "grab" => CursorIcon::Grab,
    "grabbing" => CursorIcon::Grabbing,
    "allScroll" => CursorIcon::AllScroll,
    "zoomIn" => CursorIcon::ZoomIn,
    "zoomOut" => CursorIcon::ZoomOut,
    "eResize" => CursorIcon::EResize,
    "nResize" => CursorIcon::NResize,
    "neResize" => CursorIcon::NeResize,
    "nwResize" => CursorIcon::NwResize,
    "sResize" => CursorIcon::SResize,
    "seResize" => CursorIcon::SeResize,
    "swResize" => CursorIcon::SwResize,
    "wResize" => CursorIcon::WResize,
    "ewResize" => CursorIcon::EwResize,
    "nsResize" => CursorIcon::NsResize,
    "neswResize" => CursorIcon::NeswResize,
    "nwseResize" => CursorIcon::NwseResize,
    "colResize" => CursorIcon::ColResize,
    "rowResize" => CursorIcon::RowResize,
    other => return Err(format!("invalid cursor icon '{}'", other)),
  })
}

fn progress_value(value: &Value) -> Result<ProgressBarState, String> {
  if value.is_null() {
    return Ok(ProgressBarState {
      state: Some(ProgressState::None),
      progress: None,
      desktop_filename: None,
    });
  }
  let object = value
    .as_object()
    .ok_or_else(|| "progress must be an object".to_string())?;
  let state = match object.get("state").and_then(Value::as_str).unwrap_or("normal") {
    "none" => Some(ProgressState::None),
    "normal" => Some(ProgressState::Normal),
    "indeterminate" => Some(ProgressState::Indeterminate),
    "paused" => Some(ProgressState::Paused),
    "error" => Some(ProgressState::Error),
    other => return Err(format!("invalid progress state '{}'", other)),
  };
  let progress = object.get("progress").and_then(Value::as_u64);
  let desktop_filename = object
    .get("desktopFilename")
    .and_then(Value::as_str)
    .map(ToString::to_string);
  Ok(ProgressBarState {
    state,
    progress,
    desktop_filename,
  })
}

fn window_constraints_value(value: &Value) -> Result<WindowSizeConstraints, String> {
  let object = value
    .as_object()
    .ok_or_else(|| "constraints must be an object".to_string())?;
  let mut constraints = WindowSizeConstraints::default();
  if let Some(value) = object.get("minWidth").and_then(Value::as_f64) {
    constraints.min_width = Some(tao::dpi::LogicalUnit::new(value).into());
  }
  if let Some(value) = object.get("minHeight").and_then(Value::as_f64) {
    constraints.min_height = Some(tao::dpi::LogicalUnit::new(value).into());
  }
  if let Some(value) = object.get("maxWidth").and_then(Value::as_f64) {
    constraints.max_width = Some(tao::dpi::LogicalUnit::new(value).into());
  }
  if let Some(value) = object.get("maxHeight").and_then(Value::as_f64) {
    constraints.max_height = Some(tao::dpi::LogicalUnit::new(value).into());
  }
  Ok(constraints)
}
