//! RPC 协议模块
//!
//! 在 wry 原生 IPC（`with_ipc_handler` + `evaluate_script`）之上构建结构化的 RPC 协议层。
//! 负责解析 WebView 发来的 RPC 消息、管理 Host→WebView 请求的 ID 映射，
//! 并将非 RPC 消息透传给传统的 `ipcMessage` 通道。

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// RPC 消息类型
pub enum RpcMessageType {
    /// WebView→Host 请求（request-response）
    Request,
    /// WebView 对 Host 请求的响应
    Response,
    /// WebView→Host 单向消息（fire-and-forget）
    Send,
    /// WebView 直接窗口控制（Rust 端直接处理，不转发给 Node.js）
    WinControl,
}

/// 解析后的 RPC 消息
pub struct RpcMessage {
    pub msg_type: RpcMessageType,
    pub id: Option<u64>,
    pub method: Option<String>,
    pub event: Option<String>,
    pub data: Value,
    pub error: Option<String>,
}

/// 尝试将 IPC body 解析为 RPC 消息。
/// 如果不是合法的 RPC JSON（缺少 `type` 字段或类型未知），返回 None，
/// 调用方应回退到传统的 `ipcMessage` 透传逻辑。
pub fn parse_ipc_message(body: &str) -> Option<RpcMessage> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let obj = parsed.as_object()?;
    let msg_type = match obj.get("type")?.as_str()? {
        "req" => RpcMessageType::Request,
        "res" => RpcMessageType::Response,
        "msg" => RpcMessageType::Send,
        "win" => RpcMessageType::WinControl,
        _ => return None,
    };

    Some(RpcMessage {
        msg_type,
        id: obj.get("id").and_then(Value::as_u64),
        method: obj.get("method").and_then(Value::as_str).map(String::from),
        event: obj.get("event").and_then(Value::as_str).map(String::from),
        data: obj.get("data").cloned().unwrap_or(Value::Null),
        error: obj.get("error").and_then(Value::as_str).map(String::from),
    })
}

/// RPC 回调类型
pub type RpcCallback = Box<dyn FnOnce(Result<serde_json::Value, String>) + Send>;

/// 每个窗口独立的 RPC 状态。
///
/// 仅追踪 Host→WebView 方向的请求（`pending_host_requests`）：
/// Rust 为该方向分配 rpc_id，并在 WebView 响应前持有对应的回调。
pub struct RpcState {
    host_request_counter: u64,
    /// rpc_id → (回调, 创建时间) 映射（Host→WebView 请求追踪）
    pending_host_requests: HashMap<u64, (RpcCallback, Instant)>,
}

impl Default for RpcState {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcState {
    pub fn new() -> Self {
        Self {
            host_request_counter: 0,
            pending_host_requests: HashMap::new(),
        }
    }

    /// 为 Host→WebView 请求分配新的 rpc_id，并记录回调。
    /// 返回新分配的 rpc_id。
    pub fn assign_host_request_id<F>(&mut self, callback: F) -> u64
    where
        F: FnOnce(Result<serde_json::Value, String>) + Send + 'static,
    {
        self.host_request_counter += 1;
        let rpc_id = self.host_request_counter;
        self.pending_host_requests
            .insert(rpc_id, (Box::new(callback), Instant::now()));
        rpc_id
    }

    /// WebView 响应后，移除映射并返回回调。
    pub fn resolve_host_request(&mut self, rpc_id: u64) -> Option<RpcCallback> {
        self.pending_host_requests.remove(&rpc_id).map(|(cb, _)| cb)
    }

    /// 清理所有待处理的请求（窗口关闭时调用），每个请求以 "window closed" 错误 reject。
    pub fn clear(&mut self) {
        for (_, (callback, _)) in self.pending_host_requests.drain() {
            callback(Err("window closed".to_string()));
        }
    }

    /// 清理超时的请求（超过 `max_age` 的请求），每个以 "rpc timeout" 错误 reject。
    /// 返回被清理的请求数量。
    pub fn drain_timeouts(&mut self, max_age: Duration) -> usize {
        let now = Instant::now();
        let timed_out: Vec<u64> = self
            .pending_host_requests
            .iter()
            .filter(|(_, (_, created))| now.duration_since(*created) > max_age)
            .map(|(id, _)| *id)
            .collect();
        let count = timed_out.len();
        for id in timed_out {
            if let Some((callback, _)) = self.pending_host_requests.remove(&id) {
                callback(Err("rpc timeout".to_string()));
            }
        }
        count
    }
}

// ===== 窗口控制命令队列 =====

/// 窗口控制命令队列类型
pub type WinCommandQueue = Arc<Mutex<Vec<WinCommand>>>;

/// 前端直接窗口控制命令
///
/// 由 WebView IPC handler 解析并推入队列，事件泵每帧 drain 执行。
/// setter 命令无 id（fire-and-forget），getter 命令携带 callback id 用于回传结果。
pub enum WinCommand {
    // ── Fire-and-forget ──
    Close,
    Minimize,
    Unminimize,
    Maximize,
    Unmaximize,
    Focus,
    SetVisible(bool),
    SetTitle(String),
    SetSize { width: f64, height: f64 },
    SetPosition { x: f64, y: f64 },
    SetResizable(bool),
    SetAlwaysOnTop(bool),
    SetDecorations(bool),
    Fullscreen,
    Unfullscreen,
    OpenDevtools,
    CloseDevtools,
    DragWindow,
    DragResizeWindow(String),
    SetUrl(String),
    Print,

    // ── Request-Response（携带 callback id）──
    IsMinimized(u64),
    IsMaximized(u64),
    IsFullscreen(u64),
    IsVisible(u64),
    IsResizable(u64),
    IsAlwaysOnTop(u64),
    IsDecorated(u64),
    HasFocus(u64),
    IsDevtoolsOpen(u64),
    GetSize(u64),
    GetOuterSize(u64),
    GetPosition(u64),
    GetOuterPosition(u64),
    GetTitle(u64),
    GetUrl(u64),
    GetScaleFactor(u64),
}

/// 解析窗口控制命令
///
/// 将 IPC 消息中的方法名和 JSON 数据转换为 `WinCommand` 变体。
/// 返回 `None` 表示方法名无法识别。
pub fn parse_win_command(method: &str, data: &Value, id: Option<u64>) -> Option<WinCommand> {
    match method {
        // Fire-and-forget
        "close" => Some(WinCommand::Close),
        "minimize" => Some(WinCommand::Minimize),
        "unminimize" => Some(WinCommand::Unminimize),
        "maximize" => Some(WinCommand::Maximize),
        "unmaximize" => Some(WinCommand::Unmaximize),
        "focus" => Some(WinCommand::Focus),
        "setVisible" => Some(WinCommand::SetVisible(data.as_bool().unwrap_or(true))),
        "setTitle" => Some(WinCommand::SetTitle(data.as_str().unwrap_or("").to_string())),
        "setSize" => {
            let obj = data.as_object()?;
            let w = obj.get("width")?.as_f64()?;
            let h = obj.get("height")?.as_f64()?;
            Some(WinCommand::SetSize { width: w, height: h })
        }
        "setPosition" => {
            let obj = data.as_object()?;
            let x = obj.get("x")?.as_f64()?;
            let y = obj.get("y")?.as_f64()?;
            Some(WinCommand::SetPosition { x, y })
        }
        "setResizable" => Some(WinCommand::SetResizable(data.as_bool().unwrap_or(true))),
        "setAlwaysOnTop" => Some(WinCommand::SetAlwaysOnTop(data.as_bool().unwrap_or(true))),
        "setDecorations" => Some(WinCommand::SetDecorations(data.as_bool().unwrap_or(true))),
        "fullscreen" => Some(WinCommand::Fullscreen),
        "unfullscreen" => Some(WinCommand::Unfullscreen),
        "openDevtools" => Some(WinCommand::OpenDevtools),
        "closeDevtools" => Some(WinCommand::CloseDevtools),
        "dragWindow" => Some(WinCommand::DragWindow),
        "dragResizeWindow" => Some(WinCommand::DragResizeWindow(
            data.as_str().unwrap_or("east").to_string(),
        )),
        "setUrl" => Some(WinCommand::SetUrl(data.as_str().unwrap_or("").to_string())),
        "print" => Some(WinCommand::Print),

        // Request-Response
        "isMinimized" => Some(WinCommand::IsMinimized(id?)),
        "isMaximized" => Some(WinCommand::IsMaximized(id?)),
        "isFullscreen" => Some(WinCommand::IsFullscreen(id?)),
        "isVisible" => Some(WinCommand::IsVisible(id?)),
        "isResizable" => Some(WinCommand::IsResizable(id?)),
        "isAlwaysOnTop" => Some(WinCommand::IsAlwaysOnTop(id?)),
        "isDecorated" => Some(WinCommand::IsDecorated(id?)),
        "hasFocus" => Some(WinCommand::HasFocus(id?)),
        "isDevtoolsOpen" => Some(WinCommand::IsDevtoolsOpen(id?)),
        "size" => Some(WinCommand::GetSize(id?)),
        "outerSize" => Some(WinCommand::GetOuterSize(id?)),
        "position" => Some(WinCommand::GetPosition(id?)),
        "outerPosition" => Some(WinCommand::GetOuterPosition(id?)),
        "title" => Some(WinCommand::GetTitle(id?)),
        "url" => Some(WinCommand::GetUrl(id?)),
        "scaleFactor" => Some(WinCommand::GetScaleFactor(id?)),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_called_flag() -> (Arc<AtomicBool>, impl FnOnce(Result<Value, String>)) {
        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        (flag, move |_| {
            f.store(true, Ordering::Relaxed);
        })
    }

    #[test]
    fn test_parse_request() {
        let body = r#"{"type":"req","id":1,"method":"echo","data":{"msg":"hi"}}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::Request));
        assert_eq!(msg.id, Some(1));
        assert_eq!(msg.method.as_deref(), Some("echo"));
        assert_eq!(msg.data["msg"], "hi");
    }

    #[test]
    fn test_parse_response() {
        let body = r#"{"type":"res","id":2,"data":{"count":1}}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::Response));
        assert_eq!(msg.id, Some(2));
        assert_eq!(msg.data["count"], 1);
    }

    #[test]
    fn test_parse_response_with_error() {
        let body = r#"{"type":"res","id":3,"error":"handler failed"}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::Response));
        assert_eq!(msg.error.as_deref(), Some("handler failed"));
    }

    #[test]
    fn test_parse_send() {
        let body = r#"{"type":"msg","event":"update","data":{"x":1}}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::Send));
        assert_eq!(msg.event.as_deref(), Some("update"));
    }

    #[test]
    fn test_parse_non_rpc_returns_none() {
        assert!(parse_ipc_message("hello world").is_none());
        assert!(parse_ipc_message(r#"{"foo":"bar"}"#).is_none());
        assert!(parse_ipc_message(r#"{"type":"unknown"}"#).is_none());
        assert!(parse_ipc_message("").is_none());
    }

    #[test]
    fn test_parse_win_control() {
        let body = r#"{"type":"win","method":"close"}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::WinControl));
        assert_eq!(msg.method.as_deref(), Some("close"));
    }

    #[test]
    fn test_parse_win_control_with_data() {
        let body = r#"{"type":"win","method":"setTitle","data":"hello"}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::WinControl));
        assert_eq!(msg.method.as_deref(), Some("setTitle"));
        assert_eq!(msg.data.as_str(), Some("hello"));
    }

    #[test]
    fn test_parse_win_control_with_id() {
        let body = r#"{"type":"win","id":42,"method":"isMaximized"}"#;
        let msg = parse_ipc_message(body).expect("should parse");
        assert!(matches!(msg.msg_type, RpcMessageType::WinControl));
        assert_eq!(msg.id, Some(42));
        assert_eq!(msg.method.as_deref(), Some("isMaximized"));
    }

    #[test]
    fn test_parse_win_command_setters() {
        let cmd = parse_win_command("close", &Value::Null, None);
        assert!(matches!(cmd, Some(WinCommand::Close)));

        let cmd = parse_win_command("minimize", &Value::Null, None);
        assert!(matches!(cmd, Some(WinCommand::Minimize)));

        let cmd = parse_win_command("unminimize", &Value::Null, None);
        assert!(matches!(cmd, Some(WinCommand::Unminimize)));

        let cmd = parse_win_command("maximize", &Value::Null, None);
        assert!(matches!(cmd, Some(WinCommand::Maximize)));

        let cmd = parse_win_command("unmaximize", &Value::Null, None);
        assert!(matches!(cmd, Some(WinCommand::Unmaximize)));

        let cmd = parse_win_command("setTitle", &Value::String("hi".into()), None);
        assert!(matches!(cmd, Some(WinCommand::SetTitle(s)) if s == "hi"));

        let data = serde_json::json!({"width": 800.0, "height": 600.0});
        let cmd = parse_win_command("setSize", &data, None);
        assert!(matches!(cmd, Some(WinCommand::SetSize { width, height }) if width == 800.0 && height == 600.0));
    }

    #[test]
    fn test_parse_win_command_getters() {
        let cmd = parse_win_command("isMaximized", &Value::Null, Some(7));
        assert!(matches!(cmd, Some(WinCommand::IsMaximized(7))));

        let cmd = parse_win_command("size", &Value::Null, Some(10));
        assert!(matches!(cmd, Some(WinCommand::GetSize(10))));

        // getter without id should return None
        let cmd = parse_win_command("isMaximized", &Value::Null, None);
        assert!(cmd.is_none());
    }

    #[test]
    fn test_parse_win_command_unknown() {
        let cmd = parse_win_command("unknownMethod", &Value::Null, None);
        assert!(cmd.is_none());
    }

    #[test]
    fn test_rpc_state_host_requests() {
        let mut state = RpcState::new();
        let (flag1, cb1) = make_called_flag();
        let (flag2, cb2) = make_called_flag();
        let id1 = state.assign_host_request_id(cb1);
        let id2 = state.assign_host_request_id(cb2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let cb = state.resolve_host_request(id1);
        assert!(cb.is_some());
        cb.unwrap()(Ok(Value::Null));
        assert!(flag1.load(Ordering::Relaxed));
        // 第二次 resolve 同一 ID 应返回 None
        assert!(state.resolve_host_request(id1).is_none());
        let cb2 = state.resolve_host_request(id2);
        assert!(cb2.is_some());
        cb2.unwrap()(Ok(Value::Null));
        assert!(flag2.load(Ordering::Relaxed));
    }

    #[test]
    fn test_rpc_state_clear() {
        let mut state = RpcState::new();
        let (flag, cb) = make_called_flag();
        let _id = state.assign_host_request_id(cb);
        state.clear();
        // clear() 会以 Err("window closed") 调用回调
        assert!(flag.load(Ordering::Relaxed));
        // clear 后 pending 应为空
        assert_eq!(state.drain_timeouts(Duration::from_secs(0)), 0);
    }

    #[test]
    fn test_rpc_state_drain_timeouts() {
        let mut state = RpcState::new();
        let (flag, cb) = make_called_flag();
        let _id = state.assign_host_request_id(cb);
        // 0 秒超时应立即清理
        assert_eq!(state.drain_timeouts(Duration::from_secs(0)), 1);
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_rpc_state_not_timeout_early() {
        let mut state = RpcState::new();
        let (flag, cb) = make_called_flag();
        let _id = state.assign_host_request_id(cb);
        // 使用很大的超时时间，请求不应该超时
        assert_eq!(state.drain_timeouts(Duration::from_secs(3600)), 0);
        assert!(!flag.load(Ordering::Relaxed));
    }
}
