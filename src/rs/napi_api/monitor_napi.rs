//! 显示器 napi 导出函数 — 直接执行

use napi::bindgen_prelude::*;
use serde_json::{json, Value};

use super::helpers::parse_json;

fn monitor_info(el: &tao::event_loop::EventLoopWindowTarget<crate::application::Action>, monitor: tao::monitor::MonitorHandle) -> Option<Value> {
  let id = el.available_monitors().position(|m| m == monitor)?;
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

#[napi]
fn primary_monitor() -> Result<String> {
  crate::with_app_el(|_app, el| {
    Ok(
      el.primary_monitor()
        .and_then(|m| monitor_info(el, m))
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
    )
  })
}

#[napi]
fn get_monitor_list() -> Result<String> {
  crate::with_app_el(|_app, el| {
    let monitors: Vec<Value> = el.available_monitors()
      .filter_map(|m| monitor_info(el, m))
      .collect();
    Ok(serde_json::to_string(&monitors).unwrap_or_else(|_| "[]".to_string()))
  })
}

#[napi]
fn monitor_from_point(data: String) -> Result<String> {
  let v = parse_json(&data);
  let x = v.get("x").and_then(Value::as_f64).ok_or_else(|| Error::from_reason("x is required"))?;
  let y = v.get("y").and_then(Value::as_f64).ok_or_else(|| Error::from_reason("y is required"))?;
  crate::with_app_el(|_app, el| {
    Ok(
      el.available_monitors()
        .find(|monitor| {
          let scale = monitor.scale_factor();
          let px = (x * scale) as i32;
          let py = (y * scale) as i32;
          let pos = monitor.position();
          let size = monitor.size();
          px >= pos.x && px < pos.x + size.width as i32 && py >= pos.y && py < pos.y + size.height as i32
        })
        .and_then(|m| monitor_info(el, m))
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
    )
  })
}
