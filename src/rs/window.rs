//! BrowserWindow 窗口封装
//!
//! 封装了 tao::window::Window 和 wry::WebView，提供统一的窗口操作接口。
//! 包括窗口属性、WebView 控制、光标、显示器等功能。

use image::GenericImageView;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tao::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use tao::error::{ExternalError, NotSupportedError};
use tao::monitor::MonitorHandle;
use tao::window::{
  CursorIcon, Fullscreen, Icon, ProgressBarState, ResizeDirection, Theme, UserAttentionType,
  Window as TaoWindow, WindowId, WindowSizeConstraints, RGBA,
};
use wry::{Rect, WebView};

use crate::rpc::RpcState;

/// 浏览器窗口，包含 tao 窗口和 wry WebView 的绑定
pub struct BrowserWindow {
  pub label: String,
  pub window: TaoWindow,
  pub webview: WebView,
  id: WindowId,
  pub rpc_state: Arc<Mutex<RpcState>>,
}

impl BrowserWindow {
  pub fn new(
    label: String,
    window: TaoWindow,
    webview: WebView,
    id: WindowId,
    rpc_state: Arc<Mutex<RpcState>>,
  ) -> Self {
    Self {
      label,
      window,
      webview,
      id,
      rpc_state,
    }
  }

  pub fn id(&self) -> WindowId {
    self.id
  }

  pub fn id_string(&self) -> String {
    format!("{:?}", self.id)
  }

  pub fn request_redraw(&self) {
    self.window.request_redraw();
  }

  pub fn set_url(&self, url: &str) -> wry::Result<()> {
    self.webview.load_url(url)
  }

  pub fn load_url_with_headers(
    &self,
    url: &str,
    headers: wry::http::HeaderMap,
  ) -> wry::Result<()> {
    self.webview.load_url_with_headers(url, headers)
  }

  pub fn url(&self) -> wry::Result<String> {
    self.webview.url()
  }

  pub fn evaluate_script(&self, js: &str) -> wry::Result<()> {
    // BUG FIX: wry 的 evaluate_script (fire-and-forget) 内部传入 NULL 作为 WKWebView
    // evaluateJavaScript:completionHandler: 的 completionHandler 参数。
    // 当 JS 引擎被 debugger 暂停时，WKWebView 对 NULL handler 的 evaluateJavaScript 调用
    // 处理不可靠 — 可能丢弃脚本或导致后续 evaluateJavaScript 调用无法排队。
    // 改用 evaluate_script_with_callback 并传入非 NULL 的 dummy callback，
    // 确保 WKWebView 在 debugger 暂停期间也能可靠地排队脚本，待恢复后依次执行。
    self.webview.evaluate_script_with_callback(js, |_| {})
  }

  pub fn evaluate_script_with_callback(
    &self,
    js: &str,
    callback: impl Fn(String) + Send + 'static,
  ) -> wry::Result<()> {
    self.webview.evaluate_script_with_callback(js, callback)
  }

  pub fn print(&self) -> wry::Result<()> {
    self.webview.print()
  }

  pub fn open_devtools(&self) {
    self.webview.open_devtools();
  }

  pub fn close_devtools(&self) {
    self.webview.close_devtools();
  }

  pub fn is_devtools_open(&self) -> bool {
    self.webview.is_devtools_open()
  }

  pub fn zoom(&self, scale_factor: f64) -> wry::Result<()> {
    self.webview.zoom(scale_factor)
  }

  pub fn scale_factor(&self) -> f64 {
    self.window.scale_factor()
  }

  pub fn clear_all_browsing_data(&self) -> wry::Result<()> {
    self.webview.clear_all_browsing_data()
  }

  pub fn set_background_color(&self, color: (u8, u8, u8, u8)) -> wry::Result<()> {
    self.webview.set_background_color(color)
  }

  pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    self.window.inner_position()
  }

  pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    self.window.outer_position()
  }

  pub fn set_outer_position<P: Into<Position>>(&self, position: P) {
    self.window.set_outer_position(position);
  }

  pub fn inner_size(&self) -> PhysicalSize<u32> {
    self.window.inner_size()
  }

  pub fn set_inner_size<S: Into<Size>>(&self, size: S) -> PhysicalSize<u32> {
    self.window.set_inner_size(size);
    self.window.inner_size()
  }

  pub fn outer_size(&self) -> PhysicalSize<u32> {
    self.window.outer_size()
  }

  pub fn set_min_inner_size<S: Into<Size>>(&self, min_size: Option<S>) {
    self.window.set_min_inner_size(min_size);
  }

  pub fn set_max_inner_size<S: Into<Size>>(&self, max_size: Option<S>) {
    self.window.set_max_inner_size(max_size);
  }

  pub fn set_inner_size_constraints(&self, constraints: WindowSizeConstraints) {
    self.window.set_inner_size_constraints(constraints);
  }

  pub fn set_title(&self, title: &str) {
    self.window.set_title(title);
  }

  pub fn title(&self) -> String {
    self.window.title()
  }

  pub fn set_visible(&self, visible: bool) {
    self.window.set_visible(visible);
  }

  pub fn focus_window(&self) {
    self.window.set_focus();
  }

  pub fn has_focus(&self) -> bool {
    self.window.is_focused()
  }

  pub fn set_resizable(&self, resizable: bool) {
    self.window.set_resizable(resizable);
  }

  pub fn set_minimizable(&self, minimizable: bool) {
    self.window.set_minimizable(minimizable);
  }

  pub fn set_maximizable(&self, maximizable: bool) {
    self.window.set_maximizable(maximizable);
  }

  pub fn set_closable(&self, closable: bool) {
    self.window.set_closable(closable);
  }

  pub fn set_minimized(&self, minimized: bool) {
    self.window.set_minimized(minimized);
  }

  pub fn is_minimized(&self) -> bool {
    self.window.is_minimized()
  }

  pub fn set_maximized(&self, maximized: bool) {
    self.window.set_maximized(maximized);
  }

  pub fn is_maximized(&self) -> bool {
    self.window.is_maximized()
  }

  pub fn is_visible(&self) -> bool {
    self.window.is_visible()
  }

  pub fn is_resizable(&self) -> bool {
    self.window.is_resizable()
  }

  pub fn is_minimizable(&self) -> bool {
    self.window.is_minimizable()
  }

  pub fn is_maximizable(&self) -> bool {
    self.window.is_maximizable()
  }

  pub fn is_closable(&self) -> bool {
    self.window.is_closable()
  }

  pub fn set_enabled_buttons(&self, close: bool, minimize: bool, maximize: bool) {
    self.window.set_closable(close);
    self.window.set_minimizable(minimize);
    self.window.set_maximizable(maximize);
  }

  pub fn enabled_buttons(&self) -> Vec<&'static str> {
    let mut buttons = Vec::new();
    if self.is_closable() {
      buttons.push("close");
    }
    if self.is_minimizable() {
      buttons.push("minimize");
    }
    if self.is_maximizable() {
      buttons.push("maximize");
    }
    buttons
  }

  pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
    self.window.set_fullscreen(fullscreen);
  }

  pub fn fullscreen(&self) -> Option<Fullscreen> {
    self.window.fullscreen()
  }

  pub fn set_decorations(&self, decorations: bool) {
    self.window.set_decorations(decorations);
  }

  pub fn is_decorated(&self) -> bool {
    self.window.is_decorated()
  }

  pub fn set_always_on_top(&self, always_on_top: bool) {
    self.window.set_always_on_top(always_on_top);
  }

  pub fn is_always_on_top(&self) -> bool {
    self.window.is_always_on_top()
  }

  pub fn set_always_on_bottom(&self, always_on_bottom: bool) {
    self.window.set_always_on_bottom(always_on_bottom);
  }

  pub fn set_window_background_color(&self, color: Option<RGBA>) {
    self.window.set_background_color(color);
  }

  pub fn set_window_icon(&self, icon_path: &str) -> Result<(), String> {
    self
      .window
      .set_window_icon(Some(load_tao_icon(icon_path)?));
    Ok(())
  }

  pub fn set_ime_position<P: Into<Position>>(&self, position: P) {
    self.window.set_ime_position(position);
  }

  pub fn set_progress_bar(&self, progress: ProgressBarState) {
    self.window.set_progress_bar(progress);
  }

  pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
    self.window.request_user_attention(request_type);
  }

  pub fn set_theme(&self, theme: Option<Theme>) {
    self.window.set_theme(theme);
  }

  pub fn theme(&self) -> Theme {
    self.window.theme()
  }

  pub fn set_content_protection(&self, enabled: bool) {
    self.window.set_content_protection(enabled);
  }

  pub fn set_visible_on_all_workspaces(&self, visible: bool) {
    self.window.set_visible_on_all_workspaces(visible);
  }

  pub fn set_cursor_icon(&self, cursor: CursorIcon) {
    self.window.set_cursor_icon(cursor);
  }

  pub fn set_cursor_position<P: Into<Position>>(&self, position: P) -> Result<(), ExternalError> {
    self.window.set_cursor_position(position)
  }

  pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
    self.window.set_cursor_grab(grab)
  }

  pub fn set_cursor_visible(&self, visible: bool) {
    self.window.set_cursor_visible(visible);
  }

  pub fn drag_window(&self) -> Result<(), ExternalError> {
    self.window.drag_window()
  }

  pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), ExternalError> {
    self.window.drag_resize_window(direction)
  }

  pub fn set_ignore_cursor_events(&self, ignore: bool) -> Result<(), ExternalError> {
    self.window.set_ignore_cursor_events(ignore)
  }

  pub fn cursor_position(&self) -> Result<PhysicalPosition<f64>, ExternalError> {
    self.window.cursor_position()
  }

  pub fn current_monitor(&self) -> Option<MonitorHandle> {
    self.window.current_monitor()
  }

  pub fn monitor_from_point(&self, x: f64, y: f64) -> Option<MonitorHandle> {
    self.window.monitor_from_point(x, y)
  }

  pub fn resize_webview(&self, size: Size) -> wry::Result<()> {
    self.webview.set_bounds(Rect {
      position: tao::dpi::LogicalPosition::new(0.0, 0.0).into(),
      size,
    })
  }
}

/// 加载图标文件为 tao Icon
pub fn load_tao_icon(icon_path: &str) -> Result<Icon, String> {
  let icon_image = image::open(Path::new(icon_path))
    .map_err(|error| format!("failed to load icon '{}': {}", icon_path, error))?;
  let (width, height) = icon_image.dimensions();
  let rgba_image = icon_image.to_rgba8();
  Icon::from_rgba(rgba_image.into_raw(), width, height)
    .map_err(|error| format!("invalid icon '{}': {}", icon_path, error))
}

/// 加载图标文件为 tray-icon Icon
pub fn load_tray_icon(icon_path: &str) -> Result<tray_icon::Icon, String> {
  let icon_image = image::open(Path::new(icon_path))
    .map_err(|error| format!("failed to load tray icon '{}': {}", icon_path, error))?;
  let (width, height) = icon_image.dimensions();
  let rgba_image = icon_image.to_rgba8();
  tray_icon::Icon::from_rgba(rgba_image.into_raw(), width, height)
    .map_err(|error| format!("invalid tray icon '{}': {}", icon_path, error))
}
