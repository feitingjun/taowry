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

fn install_tray_menu_handlers(proxy: EventLoopProxy<Action>) {
  let tray_proxy = proxy.clone();
  TrayIconEvent::set_event_handler(Some(move |event| {
    let _ = tray_proxy.send_event(Action::TrayIconEvent(event));
  }));
  MenuEvent::set_event_handler(Some(move |event| {
    let _ = proxy.send_event(Action::MenuEvent(event));
  }));
}
