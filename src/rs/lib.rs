#[macro_use]
extern crate napi_derive;

pub mod application;
pub mod channel;
pub mod dock;
pub mod event;
pub mod napi_api;
pub mod protocol;
pub mod rpc;
pub mod window;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::JsFunction;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tao::event::{Event, StartCause};
use tao::event_loop::ControlFlow;
use tao::platform::run_return::EventLoopExtRunReturn;

use crate::application::{Action, Application};
use crate::event::handle_window_event;
use tao::event_loop::EventLoopProxy;
use tray_icon::{TrayIconEvent, menu::MenuEvent};

// ===== 全局状态 =====

/// 退出标志
pub(crate) static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// 主线程状态：仅从 JS 主线程访问，无并发竞争。
/// 使用 static mut 因为 EventLoop 是 !Send/!Sync，必须留在主线程。
static mut MAIN: Option<MainState> = None;

struct MainState {
  app: Application,
  event_loop: tao::event_loop::EventLoop<Action>,
}

// ===== 全局状态访问器（仅在主线程调用，安全）=====

pub(crate) fn with_app<F, R>(f: F) -> Result<R>
where F: FnOnce(&mut Application) -> Result<R> {
  #[allow(static_mut_refs)]
  unsafe {
    match MAIN.as_mut() {
      Some(state) => f(&mut state.app),
      None => Err(Error::from_reason("Application not initialized")),
    }
  }
}

pub(crate) fn with_app_el<F, R>(f: F) -> Result<R>
where F: FnOnce(&mut Application, &tao::event_loop::EventLoopWindowTarget<Action>) -> Result<R> {
  #[allow(static_mut_refs)]
  unsafe {
    let state = MAIN.as_mut().ok_or_else(|| Error::from_reason("Application not initialized"))?;
    let target: &tao::event_loop::EventLoopWindowTarget<Action> = &state.event_loop;
    f(&mut state.app, target)
  }
}

// ===== napi 导出函数 =====

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
  let app = Application::new();

  install_tray_menu_handlers(proxy.clone());

  unsafe {
    MAIN = Some(MainState { app, event_loop });
  }

  QUIT_REQUESTED.store(false, Ordering::Relaxed);

  let noop = env.create_function_from_closure("noop", |_ctx| Ok(()))?;
  let pump_tsfn: ThreadsafeFunction<(), ErrorStrategy::Fatal> = noop
    .create_threadsafe_function(0, move |_ctx: ThreadSafeCallContext<()>| {
      if QUIT_REQUESTED.load(Ordering::Relaxed) {
        return Ok::<Vec<napi::JsUndefined>, napi::Error>(vec![]);
      }
      #[allow(static_mut_refs)]
      unsafe {
        if let Some(state) = MAIN.as_mut() {
          let el = &mut state.event_loop;
          let app = &mut state.app;
          let start_time = Instant::now();
          // ── Phase A: tao 事件分发 ──
          el.run_return(|event, target, control_flow| {
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
              Event::WindowEvent { window_id, event, .. } => {
                handle_window_event(app, target, window_id, event);
                *control_flow = ControlFlow::Poll;
              }
              Event::MainEventsCleared | Event::LoopDestroyed => {
                *control_flow = ControlFlow::Exit;
              }
              _ => { *control_flow = ControlFlow::Poll; }
            }
            if start_time.elapsed() > Duration::from_millis(8) {
              *control_flow = ControlFlow::Exit;
            }
          });

          // ── Phase B (macOS): 排空 NSApp 事件 + CFRunLoop 源 ──
          #[cfg(target_os = "macos")]
          drain_macos_events();
        }
      }
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
  use objc2::runtime::AnyObject;
  use objc2::rc::Retained;
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
    | NS_LEFT_MOUSE_UP | NS_RIGHT_MOUSE_DOWN | NS_RIGHT_MOUSE_UP
    | NS_MOUSE_MOVED | NS_LEFT_MOUSE_DRAGGED | NS_RIGHT_MOUSE_DRAGGED
    | NS_MOUSE_ENTERED | NS_MOUSE_EXITED | NS_KEY_DOWN | NS_KEY_UP
    | NS_FLAGS_CHANGED | NS_SCROLL_WHEEL | NS_OTHER_MOUSE_DOWN
    | NS_OTHER_MOUSE_UP | NS_OTHER_MOUSE_DRAGGED;

  let drain_mask: u64 = NS_ANY_EVENT_MASK & !user_input_mask;

  unsafe {
    let app_cls = objc2::class!(NSApplication);
    let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
    if app.is_null() { return; }

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
        } else { break; }
      }
      let result = CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
      if result == K_CF_RUN_LOOP_RUN_HANDLED_SOURCE { did_work = true; }
      if !did_work { break; }
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
