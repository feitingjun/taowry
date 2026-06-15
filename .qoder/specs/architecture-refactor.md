# taowry 架构重构计划

## Context

当前项目有 4 个问题：
1. **`app.quit()` 无效** — 后台泵线程无限运行，进程永不终止
2. **`sendCommand` 一个大函数承担所有交互** — 80+ 方法通过字符串匹配分发
3. **架构需要优化** — `application.rs` 1147 行、`command.rs` 601 行巨型文件
4. **53 条 Rust 编译警告** — deprecated cocoa、unused imports、unexpected cfg

**约束**: TS 公共 API 不变（Application, BrowserWindow, Tray, Menu, defineRPC）

---

## Task 1: 修复 app.quit()

**根因**: `quit()` 设置 `ControlFlow::Exit` 仅退出当前 `run_return`，后台泵线程每 16ms 创建新的 `run_return`，进程永不终止。

**修改**: `src/rs/lib.rs`

```rust
// 1. 添加全局退出标志
use std::sync::atomic::{AtomicBool, Ordering};
pub(crate) static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

// 2. 泵线程检查退出标志 (行 151-156)
std::thread::spawn(move || {
    while !QUIT_REQUESTED.load(Ordering::Relaxed) {
        pump_tsfn.call((), ThreadsafeFunctionCallMode::NonBlocking);
        std::thread::sleep(Duration::from_millis(16));
    }
});

// 3. 泵 TSFN 回调开头检查
if QUIT_REQUESTED.load(Ordering::Relaxed) {
    return Ok::<Vec<napi::JsUndefined>, napi::Error>(vec![]);
}

// 4. start() 中重置标志
QUIT_REQUESTED.store(false, Ordering::Relaxed);
```

**修改**: `src/rs/command.rs` quit handler

```rust
"app_quit" | "quit" => {
    crate::QUIT_REQUESTED.store(true, Ordering::Relaxed);
    *control_flow = ControlFlow::Exit;
    Ok(Value::Null)
}
```

---

## Task 2: Rust 模块拆分

### 2.1 command.rs → dispatch/ 目录

将 `command.rs`（601 行巨型 match）拆分为按领域组织的子模块：

```
src/rs/dispatch/
  mod.rs          — Command 结构体 + dispatch_command() + handle_command() 路由 + 共享辅助函数
  app_cmd.rs      — quit, app_window_labels, webview_version (~30 行)
  window_cmd.rs   — create_window + 60+ 窗口操作 (~150 行)
  menu_cmd.rs     — create_menu, append_menu_item, set_*_menu, menu_item_* (~60 行)
  tray_cmd.rs     — create_tray, remove_tray, set_tray_* (~60 行)
  dock_cmd.rs     — show/hide_dock_icon, set_dock_badge, bounce_dock, set_dock_menu (~50 行)
  monitor_cmd.rs  — primary_monitor, get_monitor_list, monitor_from_point (~30 行)
```

路由逻辑（`dispatch/mod.rs`）:
```rust
pub fn handle_command(app, cmd, event_loop, control_flow) {
    match cmd.method.as_str() {
        // 特殊命令保持原位
        "evaluate_script_with_callback" => { ... }
        "rpc_invoke" | "rpc_resolve" | "rpc_send" => { ... }
        "protocol_response" => { ... }
        // 按领域路由
        "app_quit" | "quit" | "app_window_labels" | "webview_version"
            => app_cmd::handle(app, &cmd, event_loop, control_flow),
        "create" | "create_window"
            => window_cmd::handle_create(app, &cmd, event_loop),
        m if MENU_METHODS.contains(&m)
            => menu_cmd::handle(app, &cmd, event_loop),
        m if TRAY_METHODS.contains(&m)
            => tray_cmd::handle(app, &cmd),
        m if DOCK_METHODS.contains(&m)
            => dock_cmd::handle(app, &cmd),
        m if MONITOR_METHODS.contains(&m)
            => monitor_cmd::handle(app, &cmd, event_loop),
        _ => window_cmd::handle_window_method(app, &cmd, event_loop),
    }
}
```

共享辅助函数（`bool_value`, `string_field`, `required_string`, `color_value` 等）提取到 `dispatch/mod.rs` 作为 `pub(crate)` 函数。

### 2.2 application.rs → 拆分为 4 个文件

| 新文件 | 内容 | ~行数 |
|--------|------|-------|
| `src/rs/application.rs` | `Application` 结构体核心方法 (new, create_event_loop, get/close_window, create_new_window, create_tray, remove_tray, set_tray_*, menu 操作, monitor, event handlers) | ~400 |
| `src/rs/menu_builder.rs` | `ManagedMenu`, `ManagedMenuItem`, `build_menu_item`, `append_item_to_menu`, `managed_item_id`, `parse_accelerator`, `build_predefined_item` | ~250 |
| `src/rs/webview_opts.rs` | `apply_webview_options` + `headers_from_value`, `color_from_value`, `drag_drop_payload`, `tray_event_payload` 等辅助 | ~250 |
| `src/rs/window_builder.rs` | `build_window_builder` + `size_from_value`, `position_from_value`, `theme_from_str`, `fullscreen_from_value` | ~200 |

---

## Task 3: 全部 napi 函数独立导出

### 3.1 删除 `send_command`，新增按领域独立 napi 函数

每个操作都有独立的 napi 导出。为管理数量，按领域拆分到子模块文件，`lib.rs` 统一 `pub use`。

#### 文件结构

```
src/rs/napi/
  mod.rs           — pub use 所有子模块的 napi 函数
  app_napi.rs      — quit, windowLabels, webviewVersion (3 个)
  window_napi.rs   — createWindow + 64 个窗口操作
  menu_napi.rs     — createMenu, appendMenuItem, setApplicationMenu 等 (8 个)
  tray_napi.rs     — createTray, removeTray, setTrayIcon 等 (8 个)
  dock_napi.rs     — showDockIcon, hideDockIcon, setDockBadge 等 (5 个)
  monitor_napi.rs  — primaryMonitor, getMonitorList, monitorFromPoint (3 个)
```

#### 完整 napi 函数清单

**应用级 (3 个)**:
| napi 函数名 | Rust 签名 | method |
|-------------|-----------|--------|
| `quit` | `fn quit(id: String) -> Result<()>` | `app_quit` |
| `windowLabels` | `fn window_labels(id: String) -> Result<()>` | `app_window_labels` |
| `webviewVersion` | `fn webview_version_cmd(id: String) -> Result<()>` | `webview_version` |

**窗口创建 (1 个)**:
| `createWindow` | `fn create_window(id: String, label: String, data: String) -> Result<()>` | `create_window` |

**窗口 WebView (12 个)**:
| `windowClose` | `(id, label)` | `close` |
| `windowRequestRedraw` | `(id, label)` | `request_redraw` |
| `windowSetUrl` | `(id, label, url)` | `set_url` |
| `windowLoadUrlWithHeaders` | `(id, label, data)` | `load_url_with_headers` |
| `windowUrl` | `(id, label)` | `url` |
| `windowPrint` | `(id, label)` | `print` |
| `windowOpenDevtools` | `(id, label)` | `open_devtools` |
| `windowCloseDevtools` | `(id, label)` | `close_devtools` |
| `windowIsDevtoolsOpen` | `(id, label)` | `is_devtools_open` |
| `windowZoom` | `(id, label, scale)` | `zoom` |
| `windowClearAllBrowsingData` | `(id, label)` | `clear_all_browsing_data` |
| `windowSetBackgroundColor` | `(id, label, color)` | `set_background_color` |

**窗口尺寸/位置 (12 个)**:
| `windowInnerPosition` | `(id, label)` | `inner_position` |
| `windowOuterPosition` | `(id, label)` | `outer_position` |
| `windowSetOuterPosition` | `(id, label, data)` | `set_outer_position` |
| `windowInnerSize` | `(id, label)` | `inner_size` |
| `windowSetInnerSize` | `(id, label, data)` | `set_inner_size` |
| `windowOuterSize` | `(id, label)` | `outer_size` |
| `windowSetMinInnerSize` | `(id, label, data)` | `set_min_inner_size` |
| `windowSetMaxInnerSize` | `(id, label, data)` | `set_max_inner_size` |
| `windowSetInnerSizeConstraints` | `(id, label, data)` | `set_inner_size_constraints` |
| `windowScaleFactor` | `(id, label)` | `scale_factor` |
| `windowSetWindowBackgroundColor` | `(id, label, data)` | `set_window_background_color` |
| `windowRequestUserAttention` | `(id, label, data)` | `request_user_attention` |

**窗口属性 (30 个)**:
| `windowSetTitle` | `(id, label, title)` | `set_title` |
| `windowTitle` | `(id, label)` | `title` |
| `windowSetVisible` | `(id, label, visible)` | `set_visible` |
| `windowIsVisible` | `(id, label)` | `is_visible` |
| `windowFocus` | `(id, label)` | `focus_window` |
| `windowHasFocus` | `(id, label)` | `has_focus` |
| `windowSetResizable` | `(id, label, resizable)` | `set_resizable` |
| `windowIsResizable` | `(id, label)` | `is_resizable` |
| `windowSetMinimizable` | `(id, label, v)` | `set_minimizable` |
| `windowIsMinimizable` | `(id, label)` | `is_minimizable` |
| `windowSetMaximizable` | `(id, label, v)` | `set_maximizable` |
| `windowIsMaximizable` | `(id, label)` | `is_maximizable` |
| `windowSetClosable` | `(id, label, v)` | `set_closable` |
| `windowIsClosable` | `(id, label)` | `is_closable` |
| `windowSetEnabledButtons` | `(id, label, data)` | `set_enabled_buttons` |
| `windowEnabledButtons` | `(id, label)` | `enabled_buttons` |
| `windowSetMinimized` | `(id, label, v)` | `set_minimized` |
| `windowIsMinimized` | `(id, label)` | `is_minimized` |
| `windowSetMaximized` | `(id, label, v)` | `set_maximized` |
| `windowIsMaximized` | `(id, label)` | `is_maximized` |
| `windowFullscreen` | `(id, label, data)` | `fullscreen` |
| `windowUnfullscreen` | `(id, label)` | `unfullscreen` |
| `windowIsFullscreen` | `(id, label)` | `is_fullscreen` |
| `windowSetDecorations` | `(id, label, v)` | `set_decorations` |
| `windowIsDecorated` | `(id, label)` | `is_decorated` |
| `windowSetAlwaysOnTop` | `(id, label, v)` | `set_always_on_top` |
| `windowIsAlwaysOnTop` | `(id, label)` | `is_always_on_top` |
| `windowSetAlwaysOnBottom` | `(id, label, v)` | `set_always_on_bottom` |
| `windowSetWindowIcon` | `(id, label, path)` | `set_window_icon` |
| `windowSetTheme` | `(id, label, data)` | `set_theme` |

**窗口其他 (7 个)**:
| `windowTheme` | `(id, label)` | `theme` |
| `windowSetContentProtection` | `(id, label, v)` | `set_content_protection` |
| `windowSetVisibleOnAllWorkspaces` | `(id, label, v)` | `set_visible_on_all_workspaces` |
| `windowSetImePosition` | `(id, label, data)` | `set_ime_position` |
| `windowSetProgressBar` | `(id, label, data)` | `set_progress_bar` |
| `windowId` | `(id, label)` | `id` |
| `windowEvaluateScript` | `(id, label, script)` | `evaluate_script` |

**光标 (7 个)**:
| `windowSetCursorIcon` | `(id, label, cursor)` | `set_cursor_icon` |
| `windowSetCursorPosition` | `(id, label, data)` | `set_cursor_position` |
| `windowSetCursorGrab` | `(id, label, v)` | `set_cursor_grab` |
| `windowSetCursorVisible` | `(id, label, v)` | `set_cursor_visible` |
| `windowDragWindow` | `(id, label)` | `drag_window` |
| `windowDragResizeWindow` | `(id, label, data)` | `drag_resize_window` |
| `windowSetIgnoreCursorEvents` | `(id, label, v)` | `set_ignore_cursor_events` |
| `windowCursorPosition` | `(id, label)` | `cursor_position` |

**菜单 (8 个)**:
| `createMenu` | `(id, label, data)` | `create_menu` |
| `appendMenuItem` | `(id, label, data)` | `append_menu_item` |
| `setApplicationMenu` | `(id, data)` | `set_application_menu` |
| `setWindowMenu` | `(id, label, data)` | `set_window_menu` |
| `setMenuItemEnabled` | `(id, label, data)` | `set_menu_item_enabled` |
| `setMenuItemText` | `(id, label, data)` | `set_menu_item_text` |
| `setMenuItemChecked` | `(id, label, data)` | `set_menu_item_checked` |
| `isMenuItemChecked` | `(id, label, data)` | `is_menu_item_checked` |

**托盘 (8 个)**:
| `createTray` | `(id, label, data)` | `create_tray` |
| `removeTray` | `(id, label)` | `remove_tray` |
| `setTrayIcon` | `(id, label, data)` | `set_tray_icon` |
| `setTrayMenu` | `(id, label, data)` | `set_tray_menu` |
| `setTrayTooltip` | `(id, label, data)` | `set_tray_tooltip` |
| `setTrayTitle` | `(id, label, data)` | `set_tray_title` |
| `setTrayVisible` | `(id, label, data)` | `set_tray_visible` |
| `trayRect` | `(id, label)` | `tray_rect` |

**Dock (5 个)**:
| `showDockIcon` | `(id)` | `show_dock_icon` |
| `hideDockIcon` | `(id)` | `hide_dock_icon` |
| `setDockBadge` | `(id, data)` | `set_dock_badge` |
| `bounceDock` | `(id)` | `bounce_dock` |
| `setDockMenu` | `(id, data)` | `set_dock_menu` |

**显示器 (3 个)**:
| `primaryMonitor` | `(id)` | `primary_monitor` |
| `getMonitorList` | `(id)` | `get_monitor_list` |
| `monitorFromPoint` | `(id, data)` | `monitor_from_point` |

**已有且保持不变 (7 个)**: `start`, `evaluateScript`, `rpcInvoke`, `rpcResolve`, `rpcSend`, `protocolResponse`

**总计**: ~91 个新 napi 函数 + 7 个已有 = ~98 个

### 3.2 napi 函数实现模式

每个函数遵循统一模式，通过 `dispatch_command` 发送到事件循环：

```rust
// src/rs/napi/window_napi.rs
use napi::bindgen_prelude::*;
use crate::dispatch::{Command, dispatch_command};
use serde_json::Value;

fn parse(data: &str) -> Value {
    serde_json::from_str(data).unwrap_or(Value::Null)
}

#[napi]
fn window_close(id: String, label: String) -> Result<()> {
    dispatch_command(Command { id, method: "close".into(), label, data: Value::Null })
}

#[napi]
fn window_set_title(id: String, label: String, title: String) -> Result<()> {
    dispatch_command(Command { id, method: "set_title".into(), label, data: Value::String(title) })
}

#[napi]
fn window_set_url(id: String, label: String, url: String) -> Result<()> {
    dispatch_command(Command { id, method: "set_url".into(), label, data: Value::String(url) })
}

#[napi]
fn window_set_outer_position(id: String, label: String, data: String) -> Result<()> {
    dispatch_command(Command { id, method: "set_outer_position".into(), label, data: parse(&data) })
}
// ... 其余同理
```

对于简单的无参数操作（如 `close`, `focus`），不需要 `data` 参数。
对于简单值参数（如 `set_title` 接收 string，`set_visible` 接收 bool），直接用类型化参数。
对于复杂参数（如 `set_outer_position` 接收 `{x, y}`），使用 `data: String` JSON 透传。

### 3.3 删除旧的 `send_command`

`lib.rs` 中删除 `fn send_command(...)` napi 函数。

---

## Task 4: TS 内部路由改造

### 4.1 更新 `NativeModule` 接口 (`app.ts`)

将所有新的 napi 函数加入接口定义。

### 4.2 改造 `writeMessage()` — 按 method 路由到对应 native 函数

```typescript
// method → native 函数映射表
const METHOD_MAP: Record<string, (id: string, method: string, label: string, data: string) => void> = {
  // App
  app_quit: (id, m, _, d) => native.quit(id),
  app_window_labels: (id, m, _, d) => native.windowLabels(id),
  webview_version: (id, m, _, d) => native.webviewVersionCmd(id),
  // Window creation
  create: (id, m, label, d) => native.createWindow(id, label, d),
  create_window: (id, m, label, d) => native.createWindow(id, label, d),
  // Window ops
  close: (id, m, label, _) => native.windowClose(id, label),
  set_title: (id, m, label, d) => native.windowSetTitle(id, label, d),
  set_url: (id, m, label, d) => native.windowSetUrl(id, label, d),
  // ... 完整映射
  // Menu
  create_menu: (id, m, label, d) => native.createMenu(id, label, d),
  // Tray
  create_tray: (id, m, label, d) => native.createTray(id, label, d),
  // Dock
  show_dock_icon: (id, m, _, _) => native.showDockIcon(id),
  // Monitor
  primary_monitor: (id, m, _, _) => native.primaryMonitor(id),
}

private writeMessage(msg, resolve, reject) {
  this.callbacks[msg.id] = { resolve, reject }
  try {
    const { id, method, label, data } = msg
    const json = JSON.stringify(data ?? null)
    const fn = METHOD_MAP[method]
    if (fn) {
      fn(id as string, method, label, json)
    } else {
      // fallback: 默认为窗口操作
      const windowFn = (METHOD_MAP as any)[`window_${method}`]
      if (windowFn) windowFn(id as string, method, label, json)
      else throw new Error(`Unknown method: ${method}`)
    }
  } catch (error) {
    delete this.callbacks[msg.id]
    reject(error)
  }
}
```

### 4.3 不变的文件

`window.ts`, `tray.ts`, `menu.ts`, `types.ts`, `index.ts`, `client.ts`, `utils.ts` — 无需改动。

---

## Task 5: 修复 Rust 编译警告

| 类别 | 修复方式 | 文件 |
|------|---------|------|
| 未使用导入 (handle_command, handle_window_event, mpsc, thread, Event, StartCause, ControlFlow) | 删除导入（拆分后自然消除） | application.rs |
| 未使用函数 `set_global_proxy` | 删除 | lib.rs |
| `static mut` 引用警告 | 添加 `#[allow(static_mut_refs)]` 到 pump TSFN 回调 | lib.rs |
| 已弃用 `cocoa` crate (id, nil, NSString, NSApplication 等) | 添加 `#[allow(deprecated)]` 到模块/函数级 | dock.rs, lib.rs |
| `objc` macro `unexpected_cfgs` | 添加 `#[allow(unexpected_cfgs)]` | dock.rs, lib.rs |

对 cocoa/objc 警告策略：此次用 `#[allow]` 抑制，后续独立迁移到 objc2 系列 crate。

---

## 实施顺序

### 阶段 1: 修复 app.quit()（最小改动，最高优先级）
1. lib.rs 添加 `QUIT_REQUESTED` AtomicBool
2. 修改泵线程和泵回调检查退出标志
3. 修改 quit handler 设置退出标志

### 阶段 2: Rust 模块拆分
1. 创建 `src/rs/dispatch/` 目录，将 command.rs 内容迁移到子模块
2. 拆分 application.rs 为 4 个文件
3. 删除旧的 command.rs 和 application.rs
4. 验证 `cargo build` 通过

### 阶段 3: napi 函数独立导出
1. 创建 `src/rs/napi/` 目录和子模块
2. 实现所有 ~91 个 napi 函数
3. lib.rs 中 `pub use napi::*` 并删除旧的 `send_command`
4. 验证 `cargo build` 通过

### 阶段 4: TS 内部路由改造
1. 更新 `NativeModule` 接口
2. 添加 `METHOD_MAP` 路由表
3. 改造 `writeMessage()` 逻辑
4. 验证 `npx tsc --noEmit` + 功能测试

### 阶段 5: 清理警告
1. 删除未使用导入和函数
2. 添加 `#[allow]` 到 cocoa/objc 相关代码
3. 验证 `cargo build 2>&1 | grep warning` 无未处理警告

---

## 验证方案

```bash
# 编译验证
cargo build 2>&1 | grep -E "^(error|warning:)" | head -20
npx napi build --platform
npx tsc --noEmit

# 功能验证 — app.quit()
bun -e "
const { Application, BrowserWindow } = require('./src/ts/index')
const app = new Application()
const win = new BrowserWindow('main', { url: 'https://www.baidu.com' })
app.run().then(() => {
  console.log('ready, quitting in 3s...')
  setTimeout(() => app.quit(), 3000)
})
"
# 预期: 3 秒后进程正常退出

# 功能验证 — 各命令类别
bun ./test/index.ts
# 预期: 窗口正常创建、页面加载、事件正常
```
