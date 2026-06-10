use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use serde_json::Value;
use std::sync::OnceLock;

/// 全局事件发射器（ThreadsafeFunction，Rust→JS）
/// 传递 JSON 字符串，JS 端解析
static EVENT_EMITTER: OnceLock<ThreadsafeFunction<String, ErrorStrategy::Fatal>> = OnceLock::new();

/// 初始化全局事件发射器（由 lib.rs start() 调用）
pub fn set_event_emitter(tsfn: ThreadsafeFunction<String, ErrorStrategy::Fatal>) {
  let _ = EVENT_EMITTER.set(tsfn);
}

fn get_emitter() -> Option<&'static ThreadsafeFunction<String, ErrorStrategy::Fatal>> {
  EVENT_EMITTER.get()
}

/// 发送 IO 消息到 JS 端（JSON 字符串）
fn emit(msg: Value) {
  if let Some(tsfn) = get_emitter() {
    if let Ok(json) = serde_json::to_string(&msg) {
      tsfn.call(json, ThreadsafeFunctionCallMode::NonBlocking);
    }
  }
}

/// 发送响应消息
pub fn send_response(id: &str, label: &str, method: &str, data: Value) {
  emit(serde_json::json!({
    "id": id, "type": "response", "method": method, "label": label, "data": data
  }));
}

/// 发送错误响应
pub fn send_error(id: &str, label: &str, method: &str, error: &str) {
  emit(serde_json::json!({
    "id": id, "type": "response", "method": method, "label": label, "error": error
  }));
}

/// 发送窗口事件
pub fn send_window_event(label: &str, method: &str, data: Value) {
  emit(serde_json::json!({
    "type": "windowEvent", "label": label, "method": method, "data": data
  }));
}

/// 发送应用事件
pub fn send_app_event(method: &str, data: Value) {
  emit(serde_json::json!({
    "type": "appEvent", "label": "app", "method": method, "data": data
  }));
}

/// 发送托盘事件
pub fn send_tray_event(label: &str, method: &str, data: Value) {
  emit(serde_json::json!({
    "type": "trayEvent", "label": label, "method": method, "data": data
  }));
}

/// 发送菜单事件
pub fn send_menu_event(id: &str, data: Value) {
  emit(serde_json::json!({
    "type": "menuEvent", "label": id, "method": "click", "data": data
  }));
}
