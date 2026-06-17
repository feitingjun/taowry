//! 托盘 napi 导出函数 — 直接执行

use napi::bindgen_prelude::*;
use serde_json::json;

use super::helpers::parse_json;

#[napi]
fn create_tray(label: String, data: String) -> Result<()> {
    let data = parse_json(&data);
    crate::with_app(|app| app.create_tray(label, &data).map_err(Error::from_reason))
}

#[napi]
fn remove_tray(label: String) -> Result<()> {
    crate::with_app(|app| app.remove_tray(&label).map_err(Error::from_reason))
}

#[napi]
fn set_tray_icon(label: String, icon: Buffer) -> Result<()> {
    let bytes: Vec<u8> = icon.into();
    crate::with_app(|app| {
        app.set_tray_icon_bytes(&label, &bytes)
            .map_err(Error::from_reason)
    })
}

#[napi]
fn set_tray_menu(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    let menu_label = v.as_str();
    crate::with_app(|app| {
        app.set_tray_menu(&label, menu_label)
            .map_err(Error::from_reason)
    })
}

#[napi]
fn set_tray_tooltip(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    let tooltip = v.as_str().unwrap_or("");
    crate::with_app(|app| {
        let tray = app
            .trays
            .get(&label)
            .ok_or_else(|| Error::from_reason(format!("tray '{}' does not exist", label)))?;
        let _ = tray.set_tooltip(Some(tooltip));
        Ok(())
    })
}

#[napi]
fn set_tray_title(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    let title = v.as_str().unwrap_or("");
    crate::with_app(|app| {
        let tray = app
            .trays
            .get(&label)
            .ok_or_else(|| Error::from_reason(format!("tray '{}' does not exist", label)))?;
        tray.set_title(Some(title));
        Ok(())
    })
}

#[napi]
fn set_tray_visible(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    let visible = v.as_bool().unwrap_or(true);
    crate::with_app(|app| {
        let tray = app
            .trays
            .get(&label)
            .ok_or_else(|| Error::from_reason(format!("tray '{}' does not exist", label)))?;
        let _ = tray.set_visible(visible);
        Ok(())
    })
}

#[napi]
fn tray_rect(label: String) -> Result<String> {
    crate::with_app(|app| {
        let tray = app
            .trays
            .get(&label)
            .ok_or_else(|| Error::from_reason(format!("tray '{}' does not exist", label)))?;
        match tray.rect() {
      Some(rect) => Ok(json!({"x": rect.position.x, "y": rect.position.y, "width": rect.size.width, "height": rect.size.height}).to_string()),
      None => Ok("null".to_string()),
    }
    })
}
