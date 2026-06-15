//! napi_api 共享辅助函数
//!
//! 提供 JSON 值解析、类型转换等公共工具，
//! 供所有 napi 导出函数使用。

use serde_json::{json, Value};

/// 解析 JSON 字符串，失败时返回 Null
pub(crate) fn parse_json(data: &str) -> Value { serde_json::from_str(data).unwrap_or(Value::Null) }

/// PhysicalSize → JSON 字符串
pub(crate) fn size_to_json(size: tao::dpi::PhysicalSize<u32>) -> String {
  json!({"width": size.width, "height": size.height}).to_string()
}

/// 坐标 → JSON 字符串
pub(crate) fn pos_to_json(x: f64, y: f64) -> String {
  json!({"x": x, "y": y}).to_string()
}

/// JSON → LogicalPosition
pub(crate) fn position_from_value(value: &Value) -> Option<tao::dpi::LogicalPosition<f64>> {
  crate::application::position_from_value(value)
}

/// JSON → Size (Result)
pub(crate) fn size_value(value: &Value) -> std::result::Result<tao::dpi::Size, String> {
  crate::application::size_from_value(value).ok_or_else(|| "size is invalid".to_string())
}

/// JSON → RGBA 颜色
pub(crate) fn color_value(value: &Value) -> std::result::Result<(u8, u8, u8, u8), String> {
  let array = value.as_array().ok_or_else(|| "color must be [r, g, b, a]".to_string())?;
  if array.len() != 4 { return Err("color must contain exactly 4 values".to_string()); }
  let mut values = [0u8; 4];
  for (i, item) in array.iter().enumerate() {
    let v = item.as_u64().ok_or_else(|| "color values must be integers".to_string())?;
    if v > u8::MAX as u64 { return Err("color values must be between 0 and 255".to_string()); }
    values[i] = v as u8;
  }
  Ok((values[0], values[1], values[2], values[3]))
}

/// JSON → (close, minimize, maximize) 按钮状态
pub(crate) fn enabled_buttons_value(value: &Value) -> std::result::Result<(bool, bool, bool), String> {
  let buttons = value.as_array().ok_or_else(|| "enabledButtons must be an array".to_string())?
    .iter().filter_map(Value::as_str).collect::<Vec<&str>>();
  Ok((buttons.contains(&"close"), buttons.contains(&"minimize"), buttons.contains(&"maximize")))
}

/// 字符串 → CursorIcon
pub(crate) fn cursor_icon_value(value: &str) -> std::result::Result<tao::window::CursorIcon, String> {
  Ok(match value {
    "default" => tao::window::CursorIcon::Default, "crosshair" => tao::window::CursorIcon::Crosshair, "hand" => tao::window::CursorIcon::Hand,
    "arrow" => tao::window::CursorIcon::Arrow, "move" => tao::window::CursorIcon::Move, "text" => tao::window::CursorIcon::Text,
    "wait" => tao::window::CursorIcon::Wait, "help" => tao::window::CursorIcon::Help, "progress" => tao::window::CursorIcon::Progress,
    "notAllowed" => tao::window::CursorIcon::NotAllowed, "contextMenu" => tao::window::CursorIcon::ContextMenu, "cell" => tao::window::CursorIcon::Cell,
    "verticalText" => tao::window::CursorIcon::VerticalText, "alias" => tao::window::CursorIcon::Alias, "copy" => tao::window::CursorIcon::Copy,
    "noDrop" => tao::window::CursorIcon::NoDrop, "grab" => tao::window::CursorIcon::Grab, "grabbing" => tao::window::CursorIcon::Grabbing,
    "allScroll" => tao::window::CursorIcon::AllScroll, "zoomIn" => tao::window::CursorIcon::ZoomIn, "zoomOut" => tao::window::CursorIcon::ZoomOut,
    "eResize" => tao::window::CursorIcon::EResize, "nResize" => tao::window::CursorIcon::NResize, "neResize" => tao::window::CursorIcon::NeResize,
    "nwResize" => tao::window::CursorIcon::NwResize, "sResize" => tao::window::CursorIcon::SResize, "seResize" => tao::window::CursorIcon::SeResize,
    "swResize" => tao::window::CursorIcon::SwResize, "wResize" => tao::window::CursorIcon::WResize, "ewResize" => tao::window::CursorIcon::EwResize,
    "nsResize" => tao::window::CursorIcon::NsResize, "neswResize" => tao::window::CursorIcon::NeswResize, "nwseResize" => tao::window::CursorIcon::NwseResize,
    "colResize" => tao::window::CursorIcon::ColResize, "rowResize" => tao::window::CursorIcon::RowResize,
    other => return Err(format!("invalid cursor icon '{}'", other)),
  })
}

/// 字符串 → ResizeDirection
pub(crate) fn resize_direction_value(value: &str) -> std::result::Result<tao::window::ResizeDirection, String> {
  Ok(match value {
    "east" => tao::window::ResizeDirection::East, "north" => tao::window::ResizeDirection::North,
    "northEast" => tao::window::ResizeDirection::NorthEast, "northWest" => tao::window::ResizeDirection::NorthWest,
    "south" => tao::window::ResizeDirection::South, "southEast" => tao::window::ResizeDirection::SouthEast,
    "southWest" => tao::window::ResizeDirection::SouthWest, "west" => tao::window::ResizeDirection::West,
    other => return Err(format!("invalid resize direction '{}'", other)),
  })
}

/// JSON → ProgressBarState
pub(crate) fn progress_value(value: &Value) -> std::result::Result<tao::window::ProgressBarState, String> {
  use tao::window::{ProgressBarState, ProgressState};
  if value.is_null() { return Ok(ProgressBarState { state: Some(ProgressState::None), progress: None, desktop_filename: None }); }
  let object = value.as_object().ok_or_else(|| "progress must be an object".to_string())?;
  let state = match object.get("state").and_then(Value::as_str).unwrap_or("normal") {
    "none" => Some(ProgressState::None), "normal" => Some(ProgressState::Normal),
    "indeterminate" => Some(ProgressState::Indeterminate), "paused" => Some(ProgressState::Paused),
    "error" => Some(ProgressState::Error), other => return Err(format!("invalid progress state '{}'", other)),
  };
  let progress = object.get("progress").and_then(Value::as_u64);
  let desktop_filename = object.get("desktopFilename").and_then(Value::as_str).map(ToString::to_string);
  Ok(ProgressBarState { state, progress, desktop_filename })
}

/// JSON → WindowSizeConstraints
pub(crate) fn window_constraints_value(value: &Value) -> std::result::Result<tao::window::WindowSizeConstraints, String> {
  let object = value.as_object().ok_or_else(|| "constraints must be an object".to_string())?;
  let mut c = tao::window::WindowSizeConstraints::default();
  if let Some(v) = object.get("minWidth").and_then(Value::as_f64) { c.min_width = Some(tao::dpi::LogicalUnit::new(v).into()); }
  if let Some(v) = object.get("minHeight").and_then(Value::as_f64) { c.min_height = Some(tao::dpi::LogicalUnit::new(v).into()); }
  if let Some(v) = object.get("maxWidth").and_then(Value::as_f64) { c.max_width = Some(tao::dpi::LogicalUnit::new(v).into()); }
  if let Some(v) = object.get("maxHeight").and_then(Value::as_f64) { c.max_height = Some(tao::dpi::LogicalUnit::new(v).into()); }
  Ok(c)
}

/// JSON → Option<UserAttentionType>
pub(crate) fn user_attention_value(value: &Value) -> std::result::Result<Option<tao::window::UserAttentionType>, String> {
  if value.is_null() { return Ok(None); }
  Ok(Some(match value.as_str().ok_or("expected string")? {
    "critical" => tao::window::UserAttentionType::Critical,
    "informational" => tao::window::UserAttentionType::Informational,
    other => return Err(format!("invalid user attention type '{}'", other)),
  }))
}

/// JSON → HeaderMap
pub(crate) fn headers_from_value(value: &Value) -> std::result::Result<wry::http::HeaderMap, String> {
  crate::application::headers_from_value(value)
}
