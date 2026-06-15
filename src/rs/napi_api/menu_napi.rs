//! 菜单 napi 导出函数 — 直接执行

use napi::bindgen_prelude::*;

use super::helpers::parse_json;

#[napi]
fn create_menu(label: String, data: String) -> Result<()> {
  let data = parse_json(&data);
  crate::with_app(|app| {
    app.create_menu(label, &data).map_err(Error::from_reason)
  })
}

#[napi]
fn append_menu_item(menu_label: String, data: String) -> Result<String> {
  let data = parse_json(&data);
  crate::with_app(|app| {
    app.append_menu_item(&menu_label, &data).map_err(Error::from_reason)
  })
}

#[napi]
fn set_application_menu(menu_label: String) -> Result<()> {
  crate::with_app(|app| {
    app.set_application_menu(&menu_label).map_err(Error::from_reason)
  })
}

#[napi]
fn set_window_menu(label: String, menu_label: String) -> Result<()> {
  crate::with_app(|app| {
    app.set_window_menu(&label, &menu_label).map_err(Error::from_reason)
  })
}

#[napi]
fn set_menu_item_enabled(item_id: String, enabled: bool) -> Result<()> {
  crate::with_app(|app| {
    app.set_menu_item_enabled(&item_id, enabled).map_err(Error::from_reason)
  })
}

#[napi]
fn set_menu_item_text(item_id: String, text: String) -> Result<()> {
  crate::with_app(|app| {
    app.set_menu_item_text(&item_id, &text).map_err(Error::from_reason)
  })
}

#[napi]
fn set_menu_item_checked(item_id: String, checked: bool) -> Result<()> {
  crate::with_app(|app| {
    app.set_menu_item_checked(&item_id, checked).map_err(Error::from_reason)
  })
}

#[napi]
fn is_menu_item_checked(item_id: String) -> Result<bool> {
  crate::with_app(|app| {
    app.menu_item_checked(&item_id).map_err(Error::from_reason)
  })
}
