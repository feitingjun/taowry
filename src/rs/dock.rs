//! macOS Dock 管理模块
//!
//! 提供 Dock 图标显示/隐藏、badge、bounce、dock menu 等功能。
//! 仅 macOS 平台可用。

#[cfg(target_os = "macos")]
pub mod platform {
  use cocoa::appkit::{NSApplication, NSApplicationActivationPolicy};
  use cocoa::base::{id, nil};
  use cocoa::foundation::NSString;
  use objc::runtime::Class;
  use objc::{msg_send, sel, sel_impl};

  unsafe fn shared_app() -> id {
    let cls = Class::get("NSApplication").unwrap();
    msg_send![cls, sharedApplication]
  }

  /// 设置 Dock 图标是否可见
  pub fn set_dock_visible(visible: bool) {
    unsafe {
      let app = shared_app();
      let policy = if visible {
        NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular
      } else {
        NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory
      };
      app.setActivationPolicy_(policy);
    }
  }

  /// 设置 Dock 图标 badge 文本，空字符串清除 badge
  pub fn set_dock_badge(text: &str) {
    unsafe {
      let app = shared_app();
      let ns_string = if text.is_empty() {
        nil
      } else {
        NSString::alloc(nil).init_str(text)
      };
      let dock_tile: id = msg_send![app, dockTile];
      let _: () = msg_send![dock_tile, setBadgeLabel: ns_string];
      if ns_string != nil {
        let _: () = msg_send![ns_string, autorelease];
      }
    }
  }

  /// 让 Dock 图标弹跳 (NSCriticalRequest)
  pub fn bounce_dock() {
    unsafe {
      let app = shared_app();
      let _: i64 = msg_send![app, requestUserAttention: 0i64];
    }
  }

  /// 设置 Dock 菜单
  pub fn set_dock_menu(ns_menu: *mut std::ffi::c_void) {
    unsafe {
      let app = shared_app();
      let _: () = msg_send![app, setDockMenu: ns_menu as id];
    }
  }
}
