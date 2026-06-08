//! node-webview - 基于 tao + wry 的 Node.js WebView 桥接库
//!
//! 通过 stdin/stdout IPC 协议与 Node.js 进程通信，
//! 支持创建多窗口、菜单、托盘等功能。

pub mod application;
pub mod dock;
pub mod event;
pub mod listen;
pub mod rpc;
pub mod window;

use application::Application;

fn main() {
  Application::new().run();
}
