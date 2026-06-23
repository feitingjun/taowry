#[macro_use]
extern crate napi_derive;

pub mod application;
pub mod channel;
pub mod dock;
pub mod event;
pub mod menu_manager;
pub mod napi_api;
pub mod protocol;
pub mod rpc;
pub mod tray_events;
pub mod utils;
pub mod window;
pub mod window_builder;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
    ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::JsFunction;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tao::event::{Event, StartCause};
use tao::event_loop::ControlFlow;
use tao::platform::run_return::EventLoopExtRunReturn;

use crate::application::{Action, Application};
use crate::event::handle_window_event;
use crate::rpc::WinCommand;
use crate::utils::{run_dialog_to_value};
use tao::event_loop::EventLoopProxy;
use tray_icon::{menu::MenuEvent, TrayIconEvent};

// ===== 全局状态 =====

/// 退出标志
pub(crate) static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

// 主线程状态：仅从 JS 主线程访问，无并发竞争。
// 使用 thread_local! + RefCell 替代 static mut，提供运行时借用检查。
// EventLoop 是 !Send/!Sync，必须留在主线程。
thread_local! {
    static MAIN: RefCell<Option<MainState>> = const { RefCell::new(None) };
}

struct MainState {
    app: Application,
    event_loop: tao::event_loop::EventLoop<Action>,
}

// ===== 全局状态访问器（仅在主线程调用，通过 RefCell 保证安全）=====

pub(crate) fn with_app<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Application) -> Result<R>,
{
    MAIN.with_borrow_mut(|opt| {
        let state = opt
            .as_mut()
            .ok_or_else(|| Error::from_reason("Application not initialized"))?;
        f(&mut state.app)
    })
}

pub(crate) fn with_app_el<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Application, &tao::event_loop::EventLoopWindowTarget<Action>) -> Result<R>,
{
    MAIN.with_borrow_mut(|opt| {
        let state = opt
            .as_mut()
            .ok_or_else(|| Error::from_reason("Application not initialized"))?;
        // Split borrow: app mutably, event_loop shared (via Deref to EventLoopWindowTarget)
        let app = &mut state.app;
        let target = &*state.event_loop;
        f(app, target)
    })
}

// ===== napi 导出函数 =====

/// 设置应用名称（用于应用范围目录，未设置时调用相关 API 会报错）
#[napi]
fn set_app_name(name: String) {
    utils::set_app_name(&name);
}

/// 初始化：注册事件回调，启动事件泵
#[napi]
fn start(env: Env, callback: JsFunction) -> Result<()> {
    let tsfn: ThreadsafeFunction<String, ErrorStrategy::Fatal> = callback
        .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        })?;
    channel::set_event_emitter(tsfn);

    let event_loop = Application::create_event_loop();
    let proxy = event_loop.create_proxy();
    let mut app = Application::new();
    app.proxy = Some(proxy.clone());

    install_tray_menu_handlers(proxy.clone());

    MAIN.set(Some(MainState { app, event_loop }));

    QUIT_REQUESTED.store(false, Ordering::Relaxed);

    let noop = env.create_function_from_closure("noop", |_ctx| Ok(()))?;
    let pump_tsfn: ThreadsafeFunction<(), ErrorStrategy::Fatal> =
        noop.create_threadsafe_function(0, move |_ctx: ThreadSafeCallContext<()>| {
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                return Ok::<Vec<napi::JsUndefined>, napi::Error>(vec![]);
            }
            MAIN.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    let el = &mut state.event_loop;
                    let app = &mut state.app;
                    let start_time = Instant::now();
                    // ── Phase A: tao 事件分发 ──
                    el.run_return(|event, target, control_flow| {
                        // 在 run_return 内部优先处理待处理的对话框
                        // 此时 CFRunLoop 正常运行，对话框的 runModal 能正常显示
                        process_pending_dialogs(app);

                        match event {
                            Event::NewEvents(StartCause::Init) => {
                                channel::send_app_event("ready", serde_json::Value::Null);
                                *control_flow = ControlFlow::Exit;
                            }
                            Event::UserEvent(Action::TrayIconEvent(ev)) => {
                                app.handle_tray_event_pub(ev);
                                *control_flow = ControlFlow::Poll;
                            }
                            Event::UserEvent(Action::MenuEvent(ev)) => {
                                app.handle_menu_event_pub(ev);
                                *control_flow = ControlFlow::Poll;
                            }
                            Event::UserEvent(Action::ProcessPendingDialogs) => {
                                // 在主线程 run_return 内部执行所有对话框命令
                                // 此时 CFRunLoop 正常运行，对话框的 runModal 能正常工作
                                process_pending_dialogs(app);
                                *control_flow = ControlFlow::Poll;
                            }
                            Event::WindowEvent {
                                window_id, event, ..
                            } => {
                                handle_window_event(app, target, window_id, event);
                                *control_flow = ControlFlow::Poll;
                            }
                            Event::MainEventsCleared | Event::LoopDestroyed => {
                                *control_flow = ControlFlow::Exit;
                            }
                            _ => {
                                *control_flow = ControlFlow::Poll;
                            }
                        }
                        if start_time.elapsed() > Duration::from_millis(8) {
                            *control_flow = ControlFlow::Exit;
                        }
                    });

                    // ── Phase B (macOS): 排空 NSApp 事件 + CFRunLoop 源 ──
                    #[cfg(target_os = "macos")]
                    drain_macos_events();

                    // ── Phase C: 清理超时的 RPC 请求 ──
                    for window in app.windows.values() {
                        if let Ok(mut rpc) = window.rpc_state.lock() {
                            rpc.drain_timeouts(Duration::from_secs(30));
                        }
                    }

                    // ── Phase D: 处理前端直接窗口控制命令 ──
                    let mut close_labels: Vec<String> = Vec::new();
                    for (label, window) in &app.windows {
                        let commands: Vec<WinCommand> = {
                            let mut queue = match window.cmd_queue.lock() {
                                Ok(q) => q,
                                Err(_) => continue,
                            };
                            queue.drain(..).collect()
                        };
                        for cmd in commands {
                            execute_win_command(window, cmd, &mut close_labels, label);
                        }
                    }
                    for label in close_labels {
                        app.close_window(&label);
                    }

                }
            });
            Ok::<Vec<napi::JsUndefined>, napi::Error>(vec![])
        })?;

    std::thread::spawn(move || {
        while !QUIT_REQUESTED.load(Ordering::Relaxed) {
            pump_tsfn.call((), ThreadsafeFunctionCallMode::NonBlocking);
            std::thread::sleep(Duration::from_millis(16));
        }
        std::thread::sleep(Duration::from_millis(100));
        std::process::exit(0);
    });

    Ok(())
}

// ===== macOS 事件排空 =====

#[cfg(target_os = "macos")]
fn drain_macos_events() {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSDate, NSString};

    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const std::ffi::c_void;
        fn CFRunLoopRunInMode(
            mode: *const std::ffi::c_void,
            seconds: f64,
            return_after_source_handled: u8,
        ) -> i32;
    }

    const K_CF_RUN_LOOP_RUN_HANDLED_SOURCE: i32 = 4;
    const MAX_DRAIN_ITERATIONS: usize = 500;

    const NS_LEFT_MOUSE_DOWN: u64 = 1 << 1;
    const NS_LEFT_MOUSE_UP: u64 = 1 << 2;
    const NS_RIGHT_MOUSE_DOWN: u64 = 1 << 3;
    const NS_RIGHT_MOUSE_UP: u64 = 1 << 4;
    const NS_MOUSE_MOVED: u64 = 1 << 5;
    const NS_LEFT_MOUSE_DRAGGED: u64 = 1 << 6;
    const NS_RIGHT_MOUSE_DRAGGED: u64 = 1 << 7;
    const NS_MOUSE_ENTERED: u64 = 1 << 8;
    const NS_MOUSE_EXITED: u64 = 1 << 9;
    const NS_KEY_DOWN: u64 = 1 << 10;
    const NS_KEY_UP: u64 = 1 << 11;
    const NS_FLAGS_CHANGED: u64 = 1 << 12;
    const NS_SCROLL_WHEEL: u64 = 1 << 22;
    const NS_OTHER_MOUSE_DOWN: u64 = 1 << 25;
    const NS_OTHER_MOUSE_UP: u64 = 1 << 26;
    const NS_OTHER_MOUSE_DRAGGED: u64 = 1 << 27;
    const NS_ANY_EVENT_MASK: u64 = u64::MAX;

    let user_input_mask: u64 = NS_LEFT_MOUSE_DOWN
        | NS_LEFT_MOUSE_UP
        | NS_RIGHT_MOUSE_DOWN
        | NS_RIGHT_MOUSE_UP
        | NS_MOUSE_MOVED
        | NS_LEFT_MOUSE_DRAGGED
        | NS_RIGHT_MOUSE_DRAGGED
        | NS_MOUSE_ENTERED
        | NS_MOUSE_EXITED
        | NS_KEY_DOWN
        | NS_KEY_UP
        | NS_FLAGS_CHANGED
        | NS_SCROLL_WHEEL
        | NS_OTHER_MOUSE_DOWN
        | NS_OTHER_MOUSE_UP
        | NS_OTHER_MOUSE_DRAGGED;

    let drain_mask: u64 = NS_ANY_EVENT_MASK & !user_input_mask;

    unsafe {
        let app_cls = objc2::class!(NSApplication);
        let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
        if app.is_null() {
            return;
        }

        let default_mode_str = NSString::from_str("kCFRunLoopDefaultMode");
        let distant_past: Retained<NSDate> = NSDate::distantPast();

        for _ in 0..MAX_DRAIN_ITERATIONS {
            let mut did_work = false;
            loop {
                let event: *mut AnyObject = msg_send![
                  app,
                  nextEventMatchingMask: drain_mask,
                  untilDate: &*distant_past,
                  inMode: &*default_mode_str,
                  dequeue: true
                ];
                if !event.is_null() {
                    let _: () = msg_send![app, sendEvent: event];
                    did_work = true;
                } else {
                    break;
                }
            }
            let result = CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
            if result == K_CF_RUN_LOOP_RUN_HANDLED_SOURCE {
                did_work = true;
            }
            if !did_work {
                break;
            }
        }
    }
}

fn install_tray_menu_handlers(proxy: EventLoopProxy<Action>) {
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = tray_proxy.send_event(Action::TrayIconEvent(event));
    }));
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(Action::MenuEvent(event));
    }));
}

// ===== 对话框命令处理（在 run_return 内执行）=====

/// 处理 Node 端待处理的对话框命令。
/// 必须在 run_return 的回调中调用（主线程 + CFRunLoop 运行中）。
fn process_pending_dialogs(app: &mut Application) {
    let node_dialogs: Vec<_> = std::mem::take(&mut app.pending_node_dialogs);
    for (cmd, tsfn) in node_dialogs {
        let result = run_dialog_to_value(&cmd);
        let json = serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string());
        tsfn.call(json, napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking);
    }
}

// ===== 窗口控制命令执行 =====
// ===== 窗口控制命令执行 =====

/// 执行单个窗口控制命令
fn execute_win_command(
    window: &crate::window::BrowserWindow,
    cmd: WinCommand,
    close_labels: &mut Vec<String>,
    label: &str,
) {
    use crate::rpc::WinCommand::*;
    use tao::dpi::{LogicalPosition, LogicalSize};

    match cmd {
        // ── Fire-and-forget ──
        Close => {
            close_labels.push(label.to_string());
        }
        Minimize => window.set_minimized(true),
        Unminimize => window.set_minimized(false),
        Maximize => window.set_maximized(true),
        Unmaximize => window.set_maximized(false),
        Focus => window.focus_window(),
        SetVisible(v) => window.set_visible(v),
        SetTitle(t) => window.set_title(&t),
        SetSize { width, height } => {
            window.set_inner_size(LogicalSize::new(width, height));
        }
        SetPosition { x, y } => {
            window.set_outer_position(LogicalPosition::new(x, y));
        }
        SetResizable(v) => window.set_resizable(v),
        SetAlwaysOnTop(v) => window.set_always_on_top(v),
        SetDecorations(v) => window.set_decorations(v),
        Fullscreen => window.set_fullscreen(Some(tao::window::Fullscreen::Borderless(None))),
        Unfullscreen => window.set_fullscreen(None),
        OpenDevtools => window.open_devtools(),
        CloseDevtools => window.close_devtools(),
        DragWindow => { let _ = window.drag_window(); }
        DragResizeWindow(dir) => {
            if let Ok(direction) = resize_direction_from_str(&dir) {
                let _ = window.drag_resize_window(direction);
            }
        }
        SetUrl(mut url) => {
            // 安全校验：仅允许安全的 URL 协议，防止 javascript:/data: 等攻击
            if !is_safe_url_protocol(&url) {
                return;
            }
            if url.starts_with("assets://") {
                url = url.replacen("assets://", "assets://__taowry__/", 1);
            }
            let _ = window.set_url(&url);
        }
        Print => { let _ = window.print(); }

        // ── Request-Response ──
        IsMinimized(id) => resolve_bool(window, id, window.is_minimized()),
        IsMaximized(id) => resolve_bool(window, id, window.is_maximized()),
        IsFullscreen(id) => resolve_bool(window, id, window.fullscreen().is_some()),
        IsVisible(id) => resolve_bool(window, id, window.is_visible()),
        IsResizable(id) => resolve_bool(window, id, window.is_resizable()),
        IsAlwaysOnTop(id) => resolve_bool(window, id, window.is_always_on_top()),
        IsDecorated(id) => resolve_bool(window, id, window.is_decorated()),
        HasFocus(id) => resolve_bool(window, id, window.has_focus()),
        IsDevtoolsOpen(id) => resolve_bool(window, id, window.is_devtools_open()),
        GetSize(id) => {
            let sf = window.scale_factor();
            let s = window.inner_size().to_logical::<f64>(sf);
            resolve_json(window, id, &serde_json::json!({"width": s.width, "height": s.height}));
        }
        GetOuterSize(id) => {
            let sf = window.scale_factor();
            let s = window.outer_size().to_logical::<f64>(sf);
            resolve_json(window, id, &serde_json::json!({"width": s.width, "height": s.height}));
        }
        GetPosition(id) => {
            if let Ok(p) = window.inner_position() {
                let sf = window.scale_factor();
                let lp = p.to_logical::<f64>(sf);
                resolve_json(window, id, &serde_json::json!({"x": lp.x, "y": lp.y}));
            } else {
                resolve_error(window, id, "inner_position not supported");
            }
        }
        GetOuterPosition(id) => {
            if let Ok(p) = window.outer_position() {
                let sf = window.scale_factor();
                let lp = p.to_logical::<f64>(sf);
                resolve_json(window, id, &serde_json::json!({"x": lp.x, "y": lp.y}));
            } else {
                resolve_error(window, id, "outer_position not supported");
            }
        }
        GetTitle(id) => resolve_string(window, id, &window.title()),
        GetUrl(id) => {
            match window.url() {
                Ok(u) => resolve_string(window, id, &u),
                Err(e) => resolve_error(window, id, &e.to_string()),
            }
        }
        GetScaleFactor(id) => resolve_f64(window, id, window.scale_factor()),
    }
}

/// 向 WebView 回传 bool 结果
fn resolve_bool(window: &crate::window::BrowserWindow, id: u64, value: bool) {
    let js = format!(
        "window.__taowry && window.__taowry._resolve({}, {}, null)",
        id, value
    );
    let _ = window.evaluate_script(&js);
}

/// 向 WebView 回传 f64 结果
fn resolve_f64(window: &crate::window::BrowserWindow, id: u64, value: f64) {
    let js = format!(
        "window.__taowry && window.__taowry._resolve({}, {}, null)",
        id, value
    );
    let _ = window.evaluate_script(&js);
}

/// 向 WebView 回传 JSON 结果
fn resolve_json(window: &crate::window::BrowserWindow, id: u64, value: &serde_json::Value) {
    let data = serde_json::to_string(value).unwrap_or_default();
    let js = format!(
        "window.__taowry && window.__taowry._resolve({}, {}, null)",
        id, data
    );
    let _ = window.evaluate_script(&js);
}

/// 向 WebView 回传字符串结果
fn resolve_string(window: &crate::window::BrowserWindow, id: u64, value: &str) {
    let data = serde_json::to_string(value).unwrap_or_default();
    let js = format!(
        "window.__taowry && window.__taowry._resolve({}, {}, null)",
        id, data
    );
    let _ = window.evaluate_script(&js);
}

/// 向 WebView 回传错误
fn resolve_error(window: &crate::window::BrowserWindow, id: u64, error: &str) {
    let err = serde_json::to_string(error).unwrap_or_default();
    let js = format!(
        "window.__taowry && window.__taowry._resolve({}, null, {})",
        id, err
    );
    let _ = window.evaluate_script(&js);
}

/// 字符串转换为 ResizeDirection
fn resize_direction_from_str(value: &str) -> std::result::Result<tao::window::ResizeDirection, String> {
    Ok(match value {
        "east" => tao::window::ResizeDirection::East,
        "north" => tao::window::ResizeDirection::North,
        "northEast" => tao::window::ResizeDirection::NorthEast,
        "northWest" => tao::window::ResizeDirection::NorthWest,
        "south" => tao::window::ResizeDirection::South,
        "southEast" => tao::window::ResizeDirection::SouthEast,
        "southWest" => tao::window::ResizeDirection::SouthWest,
        "west" => tao::window::ResizeDirection::West,
        other => return Err(format!("invalid resize direction '{}'", other)),
    })
}

/// 检查 URL 是否使用安全的协议
///
/// 仅允许 http/https/assets/views/file/about 协议，
/// 拒绝 javascript:/data:/vbscript: 等可执行代码的危险协议。
fn is_safe_url_protocol(url: &str) -> bool {
    let url_lower = url.to_ascii_lowercase();
    let url_lower = url_lower.trim_start();
    // 白名单协议
    url_lower.starts_with("http://")
        || url_lower.starts_with("https://")
        || url_lower.starts_with("assets://")
        || url_lower.starts_with("views://")
        || url_lower.starts_with("file://")
        || url_lower.starts_with("about:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_url_protocol_allowed() {
        assert!(is_safe_url_protocol("http://example.com"));
        assert!(is_safe_url_protocol("https://example.com"));
        assert!(is_safe_url_protocol("HTTP://EXAMPLE.COM"));
        assert!(is_safe_url_protocol("assets://__taowry__/index.html"));
        assert!(is_safe_url_protocol("views://api/data"));
        assert!(is_safe_url_protocol("file:///path/to/file.html"));
        assert!(is_safe_url_protocol("about:blank"));
        // leading whitespace should be trimmed
        assert!(is_safe_url_protocol("  https://example.com"));
    }

    #[test]
    fn test_is_safe_url_protocol_blocked() {
        // 可执行代码的危险协议
        assert!(!is_safe_url_protocol("javascript:alert(1)"));
        assert!(!is_safe_url_protocol("JAVASCRIPT:alert(1)"));
        assert!(!is_safe_url_protocol("data:text/html,<script>alert(1)</script>"));
        assert!(!is_safe_url_protocol("vbscript:msgbox"));
        // 无协议或未知协议
        assert!(!is_safe_url_protocol("example.com"));
        assert!(!is_safe_url_protocol("ftp://example.com"));
        assert!(!is_safe_url_protocol(""));
    }
}
