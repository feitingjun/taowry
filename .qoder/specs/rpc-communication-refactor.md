# RPC 通信重构：基于 wry 原生 IPC 重建 RPC 框架

## Context

当前项目的 RPC 框架完全实现在 TypeScript/JavaScript 层，Rust 仅充当"哑管道"——将所有 `window.ipc.postMessage()` 消息原封不动转发给 Node.js，并执行来自 Node.js 的原始 `evaluate_script` 字符串。所有 RPC 智能（协议解析、请求 ID 管理、回调追踪、响应路由）都在 TypeScript 中。

本次重构将 RPC 协议处理逻辑移入 Rust 层，让 Rust 的 `ipc_handler` 理解 RPC 消息类型并智能路由，使 TypeScript 侧变为薄消费者。公共 API（`defineRPC()`、`rpc.requests.method()`、`rpc.messages.event()`、`rpc.on()`、`rpc.off()`）保持不变。

wry 不提供内置 RPC 框架，仅提供底层 IPC 原语（`with_ipc_handler` + `evaluate_script`）。本方案在这些原语之上构建类 Tauri 的 RPC 协议层。

## 关键设计决策

1. **请求 ID 管理**：WebView→Host 方向的 ID 仍在桥接脚本中生成（JS Promise 生命周期要求），Host→WebView 方向的 ID 在 Rust 中生成并追踪
2. **延迟响应模式**：`rpc_invoke` 命令采用与 `evaluate_script_with_callback` 相同的延迟响应模式——Rust 不立即回复，等 WebView 响应后才发送 `_ioc:` 响应
3. **新事件类型复用 `windowEvent`**：Rust 发送的 `rpcRequest`/`rpcMessage` 事件通过现有 `windowEvent` 类型传递，避免修改 `app.ts` 的消息分发逻辑
4. **向后兼容**：非 RPC 的 `window.ipc.postMessage()` 调用继续以 `ipcMessage` 事件转发

## 修改文件清单

| 文件 | 操作 | 用途 |
|------|------|------|
| `src/rs/rpc.rs` | **新建** | RPC 协议模块：消息解析、状态管理 |
| `src/rs/main.rs` | 修改 | 注册 `pub mod rpc;` |
| `src/rs/window.rs` | 修改 | 添加 `Arc<Mutex<RpcState>>` 字段 |
| `src/rs/application.rs` | 修改 | 智能 `ipc_handler`，共享 RpcState |
| `src/rs/listen.rs` | 修改 | 处理 `rpc_invoke`/`rpc_resolve`/`rpc_send` 命令 |
| `src/ts/window.ts` | 修改 | 简化桥接脚本，替换 RPC 分发逻辑 |
| `src/ts/client.ts` | 不变 | 公共 API 不变 |
| `src/ts/types.ts` | 不变 | 类型定义不变 |
| `src/ts/app.ts` | 不变 | IO 传输层不变 |

## 详细实现步骤

### Step 1: 新建 `src/rs/rpc.rs` — RPC 协议模块

```rust
// 数据结构
pub struct RpcState {
    host_request_counter: u64,
    // rpc_id → node_ioc_id 映射（仅追踪 Host→WebView 请求）
    pending_host_requests: HashMap<u64, String>,
}

pub enum RpcMessageType {
    Request,   // webview→host 请求
    Response,  // webview 对 host 请求的响应
    Send,      // webview→host 单向消息
}

pub struct RpcMessage {
    pub msg_type: RpcMessageType,
    pub id: Option<u64>,
    pub method: Option<String>,
    pub event: Option<String>,
    pub data: Value,
    pub error: Option<String>,
}

// 解析函数
pub fn parse_ipc_message(body: &str) -> Option<RpcMessage>
// 尝试 JSON 解析，检查 "type" 字段：
//   "req" → Request 类型，提取 id/method/data
//   "res" → Response 类型，提取 id/data/error
//   "msg" → Send 类型，提取 event/data
//   其他或解析失败 → 返回 None（回退到传统 ipcMessage）

// RpcState 方法
impl RpcState {
    pub fn new() -> Self
    pub fn assign_host_request_id(&mut self, ioc_id: String) -> u64
    // 递增 counter，存储 rpc_id → ioc_id 映射，返回 rpc_id

    pub fn resolve_host_request(&mut self, rpc_id: u64) -> Option<String>
    // 移除映射，返回 ioc_id
}
```

注意：WebView→Host 方向的请求 ID 在 Rust 中只是透传（JS 生成 ID → Rust 转发给 Node.js → Node.js 原样返回 → Rust 构建 `_resolve` JS），RpcState 不需要追踪此方向的状态。

### Step 2: 修改 `src/rs/main.rs`

添加一行：
```rust
pub mod rpc;
```

### Step 3: 修改 `src/rs/window.rs`

在 `BrowserWindow` 结构体中添加字段：
```rust
use crate::rpc::RpcState;
use std::sync::{Arc, Mutex};

pub struct BrowserWindow {
    pub label: String,
    pub window: TaoWindow,
    pub webview: WebView,
    id: WindowId,
    pub rpc_state: Arc<Mutex<RpcState>>,  // 新增
}
```

修改 `new()` 方法接受 `rpc_state` 参数：
```rust
pub fn new(label: String, window: TaoWindow, webview: WebView, id: WindowId,
           rpc_state: Arc<Mutex<RpcState>>) -> Self
```

### Step 4: 修改 `src/rs/application.rs` — 智能 IPC Handler

**4a.** 修改 `create_new_window()`（当前第 155-192 行）：
- 在调用 `apply_webview_options()` 之前创建 `Arc<Mutex<RpcState>>`
- 传给 `apply_webview_options()` 和 `BrowserWindow::new()`

```rust
let rpc_state = Arc::new(Mutex::new(RpcState::new()));
let closure_rpc_state = rpc_state.clone();
webview_builder = apply_webview_options(label.clone(), webview_builder, data, closure_rpc_state)?;
// ... build webview ...
self.windows.insert(label.clone(), BrowserWindow::new(label, window, webview, id, rpc_state));
```

**4b.** 修改 `apply_webview_options()` 签名，接收 `Arc<Mutex<RpcState>>` 参数

**4c.** 替换 ipc_handler 闭包（当前第 712-724 行）：

```rust
let ipc_label = label.clone();
builder = builder.with_ipc_handler(move |request| {
    let body = request.body().clone();
    let uri = request.uri().to_string();

    if let Some(rpc_msg) = rpc_state_clone.lock().ok()
        .and_then(|_state| rpc::parse_ipc_message(&body))
    {
        match rpc_msg.msg_type {
            RpcMessageType::Request => {
                send_io_message(json!({
                    "type": "windowEvent",
                    "label": &ipc_label,
                    "method": "rpcRequest",
                    "data": {
                        "rpcId": rpc_msg.id,
                        "method": rpc_msg.method,
                        "data": rpc_msg.data
                    }
                }));
            }
            RpcMessageType::Response => {
                send_io_message(json!({
                    "type": "windowEvent",
                    "label": &ipc_label,
                    "method": "rpcResponse",
                    "data": {
                        "rpcId": rpc_msg.id,
                        "data": rpc_msg.data,
                        "error": rpc_msg.error
                    }
                }));
            }
            RpcMessageType::Send => {
                send_io_message(json!({
                    "type": "windowEvent",
                    "label": &ipc_label,
                    "method": "rpcMessage",
                    "data": {
                        "event": rpc_msg.event,
                        "data": rpc_msg.data
                    }
                }));
            }
        }
    } else {
        // 非 RPC 消息，保持向后兼容
        send_window_event(&ipc_label, "ipcMessage", json!({
            "url": uri, "body": body
        }));
    }
});
```

### Step 5: 修改 `src/rs/listen.rs` — 新命令处理器

**5a.** 在 `handle_listen()` 中添加早期拦截（在第 76 行 `evaluate_script_with_callback` 拦截之后）：

```rust
if method == "rpc_invoke" {
    handle_rpc_invoke(app, id, label, data);
    return;
}
if method == "rpc_resolve" {
    handle_rpc_resolve(app, id, label, data);
    return;
}
if method == "rpc_send" {
    handle_rpc_send(app, label, data);
    return;
}
```

**5b.** 实现 `handle_rpc_invoke()`（Host→WebView 请求）：

```rust
fn handle_rpc_invoke(app: &mut Application, ioc_id: &str, label: &str, data: &Value) {
    let result = app.get_window(label)
        .ok_or_else(|| format!("window '{}' does not exist", label))
        .and_then(|window| {
            let method = data.get("method").and_then(Value::as_str)
                .ok_or("method is required")?;
            let rpc_data = data.get("data").unwrap_or(&Value::Null);

            let rpc_id = window.rpc_state.lock()
                .map_err(|e| e.to_string())?
                .assign_host_request_id(ioc_id.to_string());

            let payload = json!({ "id": rpc_id, "method": method, "data": rpc_data });
            let js = format!(
                "window.__nodeWebview && window.__nodeWebview._handleInvoke({})",
                serde_json::to_string(&payload).unwrap()
            );
            window.evaluate_script(&js).map_err(|e| e.to_string())
        });

    if let Err(error) = result {
        send_error(ioc_id, label, "rpc_invoke", error);
    }
    // 成功时不发送响应——延迟到 WebView 回复
}
```

**5c.** 实现 `handle_rpc_resolve()`（Host 回应 WebView 请求）：

```rust
fn handle_rpc_resolve(app: &Application, id: &str, label: &str, data: &Value) {
    let result = app.get_window(label)
        .ok_or_else(|| format!("window '{}' does not exist", label))
        .and_then(|window| {
            let rpc_id = data.get("id").and_then(Value::as_u64)
                .ok_or("id is required")?;
            let rpc_data = data.get("data").unwrap_or(&Value::Null);
            let error = data.get("error").and_then(Value::as_str);

            // WebView→Host 请求 ID 仅透传，无需清理状态

            let data_json = serde_json::to_string(rpc_data).unwrap();
            let error_json = match error {
                Some(e) => serde_json::to_string(e).unwrap(),
                None => "null".to_string(),
            };
            let js = format!(
                "window.__nodeWebview && window.__nodeWebview._resolve({}, {}, {})",
                rpc_id, data_json, error_json
            );
            window.evaluate_script(&js).map_err(|e| e.to_string())
        });

    // 发送确认响应
    match result {
        Ok(_) => send_response(id, label, "rpc_resolve", Value::Null),
        Err(error) => send_error(id, label, "rpc_resolve", error),
    }
}
```

**5d.** 实现 `handle_rpc_send()`（Host→WebView 单向消息）：

```rust
fn handle_rpc_send(app: &Application, id: &str, label: &str, data: &Value) {
    let result = app.get_window(label)
        .ok_or_else(|| format!("window '{}' does not exist", label))
        .and_then(|window| {
            let event = data.get("event").and_then(Value::as_str)
                .ok_or("event is required")?;
            let rpc_data = data.get("data").unwrap_or(&Value::Null);

            let data_json = serde_json::to_string(rpc_data).unwrap();
            let js = format!(
                "window.__nodeWebview && window.__nodeWebview._handleSend(\"{}\", {})",
                event, data_json
            );
            window.evaluate_script(&js).map_err(|e| e.to_string())
        });

    match result {
        Ok(_) => send_response(id, label, "rpc_send", Value::Null),
        Err(error) => send_error(id, label, "rpc_send", error),
    }
}
```

### Step 6: 修改 `src/ts/window.ts` — 简化桥接脚本和 RPC 分发

**6a.** 替换 `RPC_BRIDGE_SCRIPT`（当前第 24-144 行，~120 行）为简化版（~50 行）：

```javascript
(function() {
  if (window.__nodeWebview) return;
  var counter = 0;
  var callbacks = {};       // WebView→Host 请求的 pending callbacks
  var rpcHandlers = {};     // Host→WebView 的 request handlers
  var messageHandlers = {}; // Host→WebView 的消息监听

  window.__nodeWebview = {
    defineRPC: function(config) {
      config = config || {};
      if (config.requests) {
        Object.keys(config.requests).forEach(function(method) {
          rpcHandlers[method] = config.requests[method];
        });
      }
      if (config.messages) {
        Object.keys(config.messages).forEach(function(event) {
          if (config.messages[event]) {
            if (!messageHandlers[event]) messageHandlers[event] = [];
            messageHandlers[event].push(config.messages[event]);
          }
        });
      }
      var rpcRequests = new Proxy({}, {
        get: function(_, method) {
          return function(data) {
            return new Promise(function(resolve, reject) {
              var id = ++counter;
              callbacks[id] = { resolve: resolve, reject: reject };
              window.ipc.postMessage(JSON.stringify({
                type: "req", id: id, method: method, data: data
              }));
            });
          };
        }
      });
      var rpcMessages = new Proxy({}, {
        get: function(_, event) {
          return function(data) {
            // 同步分发给本地 messageHandlers（保留 debugger 安全行为）
            var mh = messageHandlers[event];
            if (mh) {
              for (var i = 0; i < mh.length; i++) {
                try { mh[i](data); } catch(e) { console.error('message handler error:', e); }
              }
            }
            window.ipc.postMessage(JSON.stringify({
              type: "msg", event: event, data: data
            }));
          };
        }
      });
      return {
        requests: rpcRequests,
        messages: rpcMessages,
        on: function(event, callback) {
          if (!messageHandlers[event]) messageHandlers[event] = [];
          messageHandlers[event].push(callback);
          return function() {
            messageHandlers[event] = messageHandlers[event].filter(function(cb) {
              return cb !== callback;
            });
          };
        },
        off: function(event, callback) {
          if (!messageHandlers[event]) return;
          messageHandlers[event] = messageHandlers[event].filter(function(cb) {
            return cb !== callback;
          });
        }
      };
    },

    // 内部：resolve WebView→Host 请求的回调
    _resolve: function(id, data, error) {
      var cb = callbacks[id];
      if (!cb) return;
      delete callbacks[id];
      if (error) cb.reject(new Error(error));
      else cb.resolve(data);
    },

    // 内部：处理 Host→WebView 的 request
    _handleInvoke: function(payload) {
      var id = payload.id;
      var method = payload.method;
      var data = payload.data;
      var handler = rpcHandlers[method];
      if (!handler) {
        window.ipc.postMessage(JSON.stringify({
          type: "res", id: id, error: 'No handler for: ' + method
        }));
        return;
      }
      try {
        var result = handler(data);
        if (result && typeof result.then === 'function') {
          result.then(function(res) {
            window.ipc.postMessage(JSON.stringify({ type: "res", id: id, data: res }));
          }).catch(function(err) {
            window.ipc.postMessage(JSON.stringify({ type: "res", id: id, error: err.message || String(err) }));
          });
        } else {
          window.ipc.postMessage(JSON.stringify({ type: "res", id: id, data: result }));
        }
      } catch (err) {
        window.ipc.postMessage(JSON.stringify({ type: "res", id: id, error: err.message || String(err) }));
      }
    },

    // 内部：处理 Host→WebView 的消息（setTimeout 保留 debugger 安全修复）
    _handleSend: function(event, data) {
      setTimeout(function() {
        var mh = messageHandlers[event] || [];
        mh.forEach(function(cb) { cb(data); });
      }, 0);
    }
  };
})();
```

**6b.** 删除 `BrowserWindow` 中的以下字段和方法：
- 字段：`_webviewRpcCounter`、`_webviewRpcCallbacks`
- 方法：`handleIpcMessage()`、`resolveRpc()`、`invokeWebviewRequest()`、`sendToWebview()`
- 构造函数中的 `this.on('ipcMessage', ...)` 监听

**6c.** 在构造函数中替换 `ipcMessage` 监听为新的 RPC 事件监听：

```typescript
// 处理 Rust 转发的 WebView→Host RPC 请求
this.on('rpcRequest', (msg: { rpcId: number; method: string; data: any }) => {
  const handler = this.rpcHandlers.get(msg.method)
  if (!handler) {
    this.send('rpc_resolve', { id: msg.rpcId, data: null, error: `RPC method '${msg.method}' is not registered` })
    return
  }
  Promise.resolve()
    .then(() => handler(msg.data))
    .then(result => this.send('rpc_resolve', { id: msg.rpcId, data: result }))
    .catch(err => this.send('rpc_resolve', { id: msg.rpcId, data: null, error: err?.message || String(err) }))
})

// 处理 WebView→Host 单向消息
this.on('rpcMessage', (msg: { event: string; data: any }) => {
  const listeners = (this._rpcMessageListeners[msg.event] || []).slice()
  queueMicrotask(() => {
    for (const cb of listeners) {
      try { cb(msg.data) } catch (e) {
        console.error(`RPC message listener error [${msg.event}]:`, e)
      }
    }
  })
})
```

注意：`rpcResponse`（WebView 对 Host 请求的响应）不需要特殊处理——它通过 Rust 的延迟响应机制直接解析 `_sendIoMessage` 创建的 Promise。

**6d.** 简化 `createRPC()` 方法：

```typescript
// requests 代理：调用 WebView 端方法
const requestsProxy = new Proxy({} as any, {
  get(_, method: string) {
    return (data: any) => self.send('rpc_invoke', { method, data })
    // 直接利用 _sendIoMessage 的 Promise 机制
    // Rust 延迟响应直到 WebView 返回结果
  }
})

// messages 代理：向 WebView 发送单向消息
const messagesProxy = new Proxy({} as any, {
  get(_, event: string) {
    return (data: any) => self.send('rpc_send', { event, data })
  }
})
```

## 消息流程图

### Flow 1: WebView→Host 请求（request-response）
```
WebView JS                    Rust                        Node.js
rpc.requests.echo({msg})
  id=++counter
  callbacks[id]=cb
  ipc.postMessage({type:"req",id:1,method:"echo",data})
         │
         ▼
  ipc_handler 解析 → type:"req"
  发送 windowEvent(method:"rpcRequest",
    data:{rpcId:1,method:"echo",data})
         │                              │
         ▼                              ▼
                              on('rpcRequest') → 查找 handler
                              Promise.resolve().then(handler(data))
                              .then(result => send('rpc_resolve',
                                {id:1, data:result}))
                                    │
                                    ▼
                              stdin → rpc_resolve handler
                              evaluate_script("_resolve(1, data, null)")
                                    │
         ▼                          │
  _resolve(1, data, null)           │
  callbacks[1].resolve(data)        │
  Promise resolves ✓                │
```

### Flow 2: Host→WebView 请求（request-response）
```
Node.js                       Rust                        WebView JS
win.rpc.requests.update({msg})
  send('rpc_invoke', {method:"update", data:{msg}})
  → _sendIoMessage 创建 Promise
         │
         ▼
  handle_rpc_invoke:
    assign_host_request_id(ioc_id) → rpc_id=1
    pending: {1 → ioc_id}
    evaluate_script("_handleInvoke({id:1,...})")
    (不发送响应，延迟)
         │                              │
         ▼                              ▼
                              _handleInvoke({id:1, method:"update"})
                              handler(data) → result
                              ipc.postMessage({type:"res",id:1,data:result})
                                    │
                                    ▼
  ipc_handler 解析 → type:"res"
  resolve_host_request(1) → ioc_id
  send_response(ioc_id, data)
         │
         ▼
  response 消息到达
  _sendIoMessage Promise resolves ✓
```

### Flow 3: WebView→Host 消息（fire-and-forget）
```
WebView JS                    Rust                        Node.js
rpc.messages.update({x})
  ① 同步分发本地 messageHandlers
  ② ipc.postMessage({type:"msg",event:"update",data:{x}})
         │
         ▼
  ipc_handler 解析 → type:"msg"
  发送 windowEvent(method:"rpcMessage",
    data:{event:"update",data:{x}})
         │                              │
         ▼                              ▼
                              on('rpcMessage') → queueMicrotask
                              listeners.forEach(cb => cb(data))
```

### Flow 4: Host→WebView 消息（fire-and-forget）
```
Node.js                       Rust                        WebView JS
win.rpc.messages.update({x})
  send('rpc_send', {event:"update",data:{x}})
         │
         ▼
  handle_rpc_send:
    evaluate_script("_handleSend(\"update\", {x})")
    send_response(ack)
         │                              │
         ▼                              ▼
                              _handleSend("update", {x})
                              setTimeout(fn, 0)  ← debugger 安全
                              → messageHandlers 分发
```

## 新 IPC 协议格式

### WebView→Rust（`window.ipc.postMessage` 内容）
```
旧: { __rpc: true, id: 1, method: "echo", data: {...} }
新: { type: "req", id: 1, method: "echo", data: {...} }

旧: { __rpcResponse: true, id: 1, data: {...} }
新: { type: "res", id: 1, data: {...} }

旧: { __rpcSend: true, event: "update", data: {...} }
新: { type: "msg", event: "update", data: {...} }
```

### Rust→Node.js（stdout 事件，通过 windowEvent 传递）
```json
// WebView→Host 请求
{ "type": "windowEvent", "label": "main", "method": "rpcRequest",
  "data": { "rpcId": 1, "method": "echo", "data": {...} } }

// WebView→Host 响应（用于 Host→WebView 请求）
{ "type": "windowEvent", "label": "main", "method": "rpcResponse",
  "data": { "rpcId": 1, "data": {...}, "error": null } }

// WebView→Host 消息
{ "type": "windowEvent", "label": "main", "method": "rpcMessage",
  "data": { "event": "update", "data": {...} } }
```

### Node.js→Rust（stdin 命令）
```json
// Host→WebView 请求（延迟响应）
{ "id": "ioc-abc", "method": "rpc_invoke", "label": "main",
  "data": { "method": "update", "data": {...} } }

// Host 回应 WebView 请求
{ "method": "rpc_resolve", "label": "main",
  "data": { "id": 1, "data": {...}, "error": null } }

// Host→WebView 消息
{ "method": "rpc_send", "label": "main",
  "data": { "event": "update", "data": {...} } }
```

## 边界情况处理

1. **非 RPC 的 `window.ipc.postMessage()`**：`parse_ipc_message()` 返回 None，回退到传统 `ipcMessage` 事件
2. **WKWebView debugger 安全**：所有 `evaluate_script` 调用通过 `window.evaluate_script()` 方法，内部使用 `evaluate_script_with_callback` + dummy callback
3. **`_handleSend` debugger 安全**：保留 `setTimeout(fn, 0)` 延迟分发
4. **Host→WebView 请求失败**（evaluate_script 失败）：立即发送 error response 给 Node.js，清理 pending 状态
5. **窗口销毁时的 pending 请求**：RpcState 随 BrowserWindow 销毁，pending 请求的 Node.js Promise 会因窗口 destroy 事件而被 reject（现有的 app.ts `_unregisterWindow` 逻辑）

## 验证方案

1. **编译验证**：`cargo build` 确保 Rust 代码无编译错误
2. **TypeScript 编译**：`npx tsc --noEmit` 确保类型正确
3. **单元测试**：在 `src/rs/rpc.rs` 中测试 `parse_ipc_message()` 和 `RpcState` 操作
4. **集成测试**：使用现有的 `test/` 目录测试应用，验证四种消息流：
   - WebView 调用 Host request → 收到响应
   - Host 调用 WebView request → 收到响应
   - WebView 发送 message 给 Host → Host 收到
   - Host 发送 message 给 WebView → WebView 收到
5. **向后兼容测试**：验证 `examples/ipc_test.rs` 仍可工作（raw IPC 无 RPC 协议）
6. **debugger 测试**：在 WebView 的 message handler 中添加 `debugger` 语句，验证 Host 的 `_resolve` 不被阻塞
