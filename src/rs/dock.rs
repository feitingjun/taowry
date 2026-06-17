//! macOS Dock 管理模块
//!
//! 提供 Dock 图标显示/隐藏、badge、bounce、dock menu 等功能。
//! 仅 macOS 平台可用。

#[cfg(target_os = "macos")]
pub mod platform {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;

    unsafe fn shared_app() -> *mut AnyObject {
        let cls = objc2::class!(NSApplication);
        unsafe { msg_send![cls, sharedApplication] }
    }

    /// 设置 Dock 图标是否可见
    pub fn set_dock_visible(visible: bool) {
        unsafe {
            let app = shared_app();
            // NSApplicationActivationPolicyRegular = 0, NSApplicationActivationPolicyAccessory = 1
            let policy: i64 = if visible { 0 } else { 1 };
            let _: () = msg_send![app, setActivationPolicy: policy];
        }
    }

    /// 设置 Dock 图标 badge 文本，空字符串清除 badge
    pub fn set_dock_badge(text: &str) {
        unsafe {
            let app = shared_app();
            let ns_string: Retained<NSString> = NSString::from_str(text);
            let badge: Option<&NSString> = if text.is_empty() {
                None
            } else {
                Some(&ns_string)
            };
            let dock_tile: *mut AnyObject = msg_send![app, dockTile];
            let _: () = msg_send![dock_tile, setBadgeLabel: badge];
        }
    }

    /// 让 Dock 图标弹跳 (NSCriticalRequest = 0)
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
            let _: () = msg_send![app, setDockMenu: ns_menu as *mut AnyObject];
        }
    }
}
