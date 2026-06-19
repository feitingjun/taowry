//! 自定义协议模块
//!
//! 管理每个窗口 pending 的 `views://` 协议请求。
//! 当 WebView 发起 `views://` 请求时，wry 的异步 protocol handler 将
//! `RequestAsyncResponder` 存入 `ProtocolState`，等待 Node 端处理后通过
//! `protocol_response` IPC 命令调用 `respond()` 返回响应。

use std::collections::HashMap;
use wry::RequestAsyncResponder;

/// 每个窗口独立的协议请求状态管理
pub struct ProtocolState {
    /// request_id → responder 映射
    pending: HashMap<String, RequestAsyncResponder>,
}

impl Default for ProtocolState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolState {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// 存储一个新的 pending 请求，返回唯一的 request_id
    pub fn insert(&mut self, responder: RequestAsyncResponder) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.pending.insert(id.clone(), responder);
        id
    }

    /// 消费一个 pending 请求，返回 responder（如果存在）
    pub fn remove(&mut self, id: &str) -> Option<RequestAsyncResponder> {
        self.pending.remove(id)
    }

    /// 清理所有待处理的协议请求（窗口关闭时调用）
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}
