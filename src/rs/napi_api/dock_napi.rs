//! Dock napi 导出函数 — 直接执行 (仅 macOS)

use napi::bindgen_prelude::*;

#[napi]
fn show_dock_icon() -> Result<()> {
    #[cfg(target_os = "macos")]
    crate::dock::platform::set_dock_visible(true);
    Ok(())
}

#[napi]
fn hide_dock_icon() -> Result<()> {
    #[cfg(target_os = "macos")]
    crate::dock::platform::set_dock_visible(false);
    Ok(())
}

#[napi]
fn set_dock_badge(text: String) -> Result<()> {
    #[cfg(target_os = "macos")]
    crate::dock::platform::set_dock_badge(&text);
    let _ = text;
    Ok(())
}

#[napi]
fn bounce_dock() -> Result<()> {
    #[cfg(target_os = "macos")]
    crate::dock::platform::bounce_dock();
    Ok(())
}

#[napi]
fn set_dock_menu(menu_label: String) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        crate::with_app(|app| app.set_dock_menu(&menu_label).map_err(Error::from_reason))?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = menu_label;
    }
    Ok(())
}
