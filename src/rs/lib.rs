#[macro_use]
extern crate napi_derive;

pub mod application;
pub mod channel;
pub mod command;
pub mod dock;
pub mod event;
pub mod protocol;
pub mod rpc;
pub mod window;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use tao::platform::run_return::EventLoopExtRunReturn;
use napi::JsFunction;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tao::event::{Event, StartCause};
use tao::event_loop::ControlFlow;

use crate::application::{Action, Application};
use crate::command::{handle_command, Command};
use crate::event::handle_window_event;
use tao::event_loop::EventLoopProxy;
use tray_icon::{TrayIconEvent, menu::MenuEvent};

// ===== 全局状态 =====
static mut GLOBAL_PROXY: Option<EventLoopProxy<Action>> = None;
static mut GLOBAL_CMD_TX: Option<mpsc::Sender<Command>> = None;

// 主线程状态（仅在主线程访问，ThreadsafeFunction 回调保证）
static mut MAIN_EVENT_LOOP: Option<tao::event_loop::EventLoop<Action>> = None;
static mut MAIN_CMD_RX: Option<mpsc::Receiver<Command>> = None;
static mut MAIN_APP: Option<Application> = None;

/// 使闭包可 Send（值仅在主线程访问，安全）
struct UnsafeSend<T>(T);
unsafe impl<T> Send for UnsafeSend<T> {}

// ===== 内部辅助 =====

/// 发送命令到事件循环
fn dispatch_command(cmd: Command) -> Result<()> {
  unsafe {
    if let Some(proxy) = &GLOBAL_PROXY {
      proxy.send_event(Action::ForwardCommand(cmd))
        .map_err(|e| Error::from_reason(format!("Failed to send command: {}", e)))?;
    } else if let Some(tx) = &GLOBAL_CMD_TX {
      tx.send(cmd).map_err(|e| Error::from_reason(format!("Failed to send command: {}", e)))?;
    } else {
      return Err(Error::from_reason("Application not started"));
    }
  }
  Ok(())
}

// ===== napi 导出函数 =====

/// 初始化：注册事件回调，启动 tao 事件循环
#[napi]
fn start(env: Env, callback: JsFunction) -> Result<()> {
  // 事件通知 TSFN（Rust→JS，JSON 字符串）
  let tsfn: ThreadsafeFunction<String, ErrorStrategy::Fatal> = callback
    .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<String>| {
      ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
    })?;
  channel::set_event_emitter(tsfn);

  let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
  let event_loop = Application::create_event_loop();
  let proxy = event_loop.create_proxy();
  let app = Application::new();

  install_tray_menu_handlers(proxy.clone());

  unsafe {
    GLOBAL_PROXY = Some(proxy.clone());
    GLOBAL_CMD_TX = Some(cmd_tx);
    MAIN_EVENT_LOOP = Some(event_loop);
    MAIN_CMD_RX = Some(cmd_rx);
    MAIN_APP = Some(app);
  }

  // pump TSFN：定时在主线程驱动事件循环
  let noop = env.create_function_from_closure("noop", |_ctx| Ok(()))?;
  let _guard = UnsafeSend(());
  let pump_tsfn: ThreadsafeFunction<(), ErrorStrategy::Fatal> = noop
    .create_threadsafe_function(0, move |_ctx: ThreadSafeCallContext<()>| {
      let _g = &_guard;
      unsafe {
        // 转发所有待处理命令到事件循环
        if let (Some(rx), Some(proxy)) = (&MAIN_CMD_RX, &GLOBAL_PROXY) {
          while let Ok(cmd) = rx.try_recv() {
            let _ = proxy.send_event(Action::ForwardCommand(cmd));
          }
        }
        // 驱动 tao 事件循环，处理所有待处理事件后返回
        if let (Some(el), Some(app)) = (&mut MAIN_EVENT_LOOP, &mut MAIN_APP) {
          let start_time = Instant::now();
          // ── Phase A: tao 事件分发 ──
          el.run_return(|event, target, control_flow| {
            match event {
              Event::NewEvents(StartCause::Init) => {
                channel::send_app_event("ready", serde_json::Value::Null);
                *control_flow = ControlFlow::Exit;
              }
              Event::UserEvent(Action::ForwardCommand(cmd)) => {
                handle_command(app, cmd, target, control_flow);
                // 处理完用户事件后继续处理其他事件
                *control_flow = ControlFlow::Poll;
              }
              Event::UserEvent(Action::TrayIconEvent(ev)) => {
                app.handle_tray_event_pub(ev);
                *control_flow = ControlFlow::Poll;
              }
              Event::UserEvent(Action::MenuEvent(ev)) => {
                app.handle_menu_event_pub(ev);
                *control_flow = ControlFlow::Poll;
              }
              Event::WindowEvent { window_id, event, .. } => {
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
            // 安全超时：单次 pump 最多处理 8ms，避免阻塞 Node 事件循环
            if start_time.elapsed() > Duration::from_millis(8) {
              *control_flow = ControlFlow::Exit;
            }
          });

          // ── Phase B (macOS): 排空 NSApp 事件 + CFRunLoop 源 ──
          // WebKit (WKWebView) 依赖 NSApp 事件和 CFRunLoop 源（GCD dispatch、
          // Mach port 通知）进行内部处理（网络→解析→渲染级联）。
          // tao 的 run_return 只处理一轮事件就退出，留下未处理的 WebKit 工作。
          // 没有这个 drain，每个级联步骤都要等 16ms 下一次 pump，导致页面无法加载。
          #[cfg(target_os = "macos")]
          drain_macos_events();
        }
      }
      Ok::<Vec<napi::JsUndefined>, napi::Error>(vec![])
    })?;

  // 后台线程 ~60fps 触发 pump
  std::thread::spawn(move || {
    loop {
      pump_tsfn.call((), ThreadsafeFunctionCallMode::NonBlocking);
      std::thread::sleep(Duration::from_millis(16));
    }
  });

  Ok(())
}

/// 通用命令（窗口属性、菜单、托盘、Dock、显示器等操作）
#[napi]
fn send_command(id: String, method: String, label: String, data: String) -> Result<()> {
  let data_value: serde_json::Value =
    serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);
  dispatch_command(Command { id, method, label, data: data_value })
}

/// 执行 WebView JS 脚本并异步返回结果
#[napi]
fn evaluate_script(id: String, label: String, script: String) -> Result<()> {
  dispatch_command(Command {
    id,
    method: "evaluate_script_with_callback".to_string(),
    label,
    data: serde_json::Value::String(script),
  })
}

/// Host→WebView RPC 请求（延迟响应：WebView 回复后才返回结果）
#[napi]
fn rpc_invoke(id: String, label: String, method: String, data: String) -> Result<()> {
  let data_value: serde_json::Value =
    serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);
  dispatch_command(Command {
    id,
    method: "rpc_invoke".to_string(),
    label,
    data: serde_json::json!({ "method": method, "data": data_value }),
  })
}

/// Host 回复 WebView→Host 的 RPC 请求
#[napi]
fn rpc_resolve(id: String, label: String, rpc_id: i64, data: String, error: Option<String>) -> Result<()> {
  let data_value: serde_json::Value =
    serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);
  let mut payload = serde_json::json!({ "id": rpc_id, "data": data_value });
  if let Some(err) = error {
    payload["error"] = serde_json::Value::String(err);
  }
  dispatch_command(Command {
    id,
    method: "rpc_resolve".to_string(),
    label,
    data: payload,
  })
}

/// Host→WebView 单向 RPC 消息（fire-and-forget）
#[napi]
fn rpc_send(id: String, label: String, event: String, data: String) -> Result<()> {
  let data_value: serde_json::Value =
    serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);
  dispatch_command(Command {
    id,
    method: "rpc_send".to_string(),
    label,
    data: serde_json::json!({ "event": event, "data": data_value }),
  })
}

/// 回复 views:// 自定义协议请求
#[napi]
fn protocol_response(id: String, label: String, request_id: String, status_code: u32, headers: String, body: String) -> Result<()> {
  let headers_value: serde_json::Value =
    serde_json::from_str(&headers).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
  dispatch_command(Command {
    id,
    method: "protocol_response".to_string(),
    label,
    data: serde_json::json!({
      "requestId": request_id,
      "statusCode": status_code,
      "headers": headers_value,
      "data": body
    }),
  })
}

// ===== 内部函数 =====

pub(crate) fn set_global_proxy(proxy: EventLoopProxy<Action>) {
  unsafe { GLOBAL_PROXY = Some(proxy); }
}

/// 排空 macOS NSApp 非输入类事件 + CFRunLoop 源，确保 WebKit 级联事件被处理。
///
/// WebKit (WKWebView) 依赖 NSApp 事件和 CFRunLoop 源（GCD dispatch、Mach port
/// 通知）进行内部处理。tao 的 run_return 处理一轮事件后就通过 [NSApp stop:] 退出，
/// 留下未处理的 WebKit 工作。没有这个 drain，网络→解析→渲染级联的每一步都要等
/// 16ms 下一次 pump 调用，导致内容渲染延迟甚至页面无法加载。
///
/// 交替执行：
///   1. 排空立即可用的**非输入类** NSApp 事件
///   2. 处理一个挂起的 CFRunLoop 源（CFRunLoopRunInMode）
/// 直到两个队列都为空。
///
/// **重要：** 鼠标和键盘事件被排除在 drain mask 之外。
/// 它们必须通过 tao 的 run_return（Phase A）处理，以确保 tao 的事件处理器
/// 在窗口委托回调（如 windowShouldClose:）触发时处于活跃状态。
#[cfg(target_os = "macos")]
fn drain_macos_events() {
  use cocoa::base::{id, nil};
  use cocoa::foundation::NSString;
  use objc::{msg_send, sel, sel_impl};

  // CoreFoundation FFI — 处理 GCD/Mach-port 源
  // CoreFoundation.framework 在 macOS 上始终可用，无需额外依赖
  unsafe extern "C" {
    static kCFRunLoopDefaultMode: *const std::ffi::c_void;
    fn CFRunLoopRunInMode(
      mode: *const std::ffi::c_void,
      seconds: f64,
      return_after_source_handled: u8,
    ) -> i32;
  }

  /// CFRunLoopRunInMode 返回值：已处理一个源
  const K_CF_RUN_LOOP_RUN_HANDLED_SOURCE: i32 = 4;
  /// 安全上限：防止源不断产生新工作时无限循环
  const MAX_DRAIN_ITERATIONS: usize = 500;

  // NSEventMask 常量（u64 位掩码）
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

  // 用户输入事件掩码 — 这些事件留在队列中由 Phase A (run_return) 处理
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

  // drain mask = 所有事件 - 用户输入事件
  let drain_mask: u64 = NS_ANY_EVENT_MASK & !user_input_mask;

  unsafe {
    let app: id = msg_send![
      objc::runtime::Class::get("NSApplication").unwrap(),
      sharedApplication
    ];
    if app == nil {
      return;
    }

    // NSDefaultRunLoopMode = @"kCFRunLoopDefaultMode"
    let default_mode_str: id = NSString::alloc(nil).init_str("kCFRunLoopDefaultMode");

    // [NSDate distantPast] — 确保不阻塞，只取立即可用的事件
    let distant_past: id = msg_send![
      objc::runtime::Class::get("NSDate").unwrap(),
      distantPast
    ];

    for _ in 0..MAX_DRAIN_ITERATIONS {
      let mut did_work = false;

      // Phase 1: 排空非输入类 NSApp 事件
      loop {
        let event: id = msg_send![
          app,
          nextEventMatchingMask: drain_mask
          untilDate: distant_past
          inMode: default_mode_str
          dequeue: true
        ];
        if event != nil {
          let _: () = msg_send![app, sendEvent: event];
          did_work = true;
        } else {
          break;
        }
      }

      // Phase 2: 处理一个挂起的 CFRunLoop 源
      //（GCD dispatch blocks、Mach-port 通知、定时器）
      let result = CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
      if result == K_CF_RUN_LOOP_RUN_HANDLED_SOURCE {
        did_work = true;
      }

      if !did_work {
        break;
      }
    }

    // 释放创建的 NSString
    if default_mode_str != nil {
      let _: () = msg_send![default_mode_str, autorelease];
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
