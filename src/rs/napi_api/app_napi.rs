//! 应用级 napi 导出函数 — 直接执行

use napi::bindgen_prelude::*;
use std::sync::atomic::Ordering;

use super::helpers::parse_json;

/// 退出应用
#[napi]
fn quit() -> Result<()> {
  crate::QUIT_REQUESTED.store(true, Ordering::Relaxed);
  Ok(())
}

/// 获取所有窗口标签
#[napi]
fn window_labels() -> Result<String> {
  crate::with_app(|app| {
    let labels: Vec<&String> = app.windows.keys().collect();
    Ok(serde_json::to_string(&labels).unwrap_or_default())
  })
}

/// 获取 WebView 引擎版本号
#[napi]
fn webview_version() -> Result<String> {
  Ok(wry::webview_version().unwrap_or_else(|e| format!("unknown: {}", e)))
}

/// 创建新窗口
#[napi]
fn create_window(label: String, data: String) -> Result<String> {
  let data = parse_json(&data);
  crate::with_app_el(|app, el| {
    let builder = crate::application::build_window_builder(app, el, &data)
      .map_err(Error::from_reason)?;
    let id = app.create_new_window(el, label, builder, &data)
      .map_err(Error::from_reason)?;
    Ok(id)
  })
}
