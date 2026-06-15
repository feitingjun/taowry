# [taowry](https://github.com/feitingjun/taowry)

基于 **tao**（跨平台窗口管理）+ **wry**（WebView 渲染）的跨平台 WebView 窗口库，支持 Node.js 和 Bun。

https://github.com/feitingjun/taowry

## 目录

- [安装](#安装)
- [快速开始](#快速开始)
- [Application](#application)
- [BrowserWindow](#browserwindow)
  - [创建窗口](#创建窗口)
  - [窗口事件](#窗口事件)
  - [RPC 通信](#rpc-通信)
  - [views:// 自定义协议](#views-自定义协议)
  - [WebView 操作](#webview-操作)
  - [窗口位置与尺寸](#窗口位置与尺寸)
  - [窗口属性](#窗口属性)
  - [菜单](#菜单)
  - [全屏与装饰](#全屏与装饰)
  - [外观与行为](#外观与行为)
  - [光标](#光标)
- [Tray](#tray)
- [Menu 菜单配置](#menu-菜单配置)
- [getWindow](#getwindow)
- [defineRPC](#definerpc)
- [BrowserWindowAttributes](#browserwindowattributes)
- [WindowEvent](#windowevent)
- [类型定义](#类型定义)
- [注意事项](#注意事项)

---

## 安装

```bash
npm install taowry
```

安装时会自动通过 postinstall 脚本下载对应平台的二进制文件，无需运行时下载。
使用 pkg、bun build 等工具打包时，二进制文件会一并打包进去。

## 快速开始

```typescript
import { Application, BrowserWindow } from 'taowry'

// 1. 创建应用实例（构造时即启动 Rust 事件循环）
const app = new Application()

// 2. 创建窗口（同步）
const win = new BrowserWindow('main', {
  url: 'https://example.com',
  width: 1024,
  height: 768,
  title: 'My App',
})

// 3. 监听窗口事件
win.onCreated((id) => {
  console.log('窗口已创建:', id)
})

// 4. 监听应用事件
app.on('ready', () => console.log('应用已就绪'))

// 5. 窗口操作（同步调用，立即返回）
win.setTitle('新标题')
console.log(win.title())
```

---

## Application

应用实例管理器，构造时即启动 Rust 事件循环线程，无需调用 `run()`。

```typescript
import { Application } from 'taowry'

const app = new Application()
```

> 同一时间只能存在一个 Application 实例，重复创建会抛出错误。

### ready

应用是否就绪（`ready` 事件触发后为 `true`）。

```typescript
app.ready: boolean
```

### whenReady()

等待应用就绪，如果已就绪则立即 resolve。

```typescript
await app.whenReady(): Promise<void>
```

### quit()

退出应用。

```typescript
app.quit(): void
```

### webviewVersion()

获取 WebView 引擎版本号。

```typescript
const version = app.webviewVersion()
// => "21624.2.5.11.4"
```

### windowLabels()

获取所有窗口标签列表。

```typescript
const labels = app.windowLabels()
// => ["main", "settings"]
```

### getWindow(label)

通过标签名获取已创建的窗口实例。

```typescript
const win = app.getWindow('main')
```

### setApplicationMenu(menu)

设置应用全局菜单（仅 macOS 有效）。

```typescript
await app.setApplicationMenu([
  {
    text: '文件',
    items: [
      { text: '新建', accelerator: 'CmdOrCtrl+N' },
      { type: 'separator' },
      { text: '退出', type: 'predefined', item: 'quit' },
    ]
  },
])
```

### setDockMenu(menu)

设置 Dock 菜单（仅 macOS 有效）。

```typescript
await app.setDockMenu([
  { text: '快速操作' },
])
```

### Dock 方法（仅 macOS）

| 方法 | 说明 |
|------|------|
| `showDockIcon()` | 显示 Dock 图标 |
| `hideDockIcon()` | 隐藏 Dock 图标 |
| `setDockBadge(text)` | 设置 Dock badge 文本，空字符串清除 |
| `bounceDock()` | 让 Dock 图标弹跳 |

### 显示器

| 方法 | 说明 | 返回值 |
|------|------|--------|
| `monitors()` | 获取所有显示器 | `Monitor[]` |
| `primaryMonitor()` | 获取主显示器 | `Monitor \| null` |
| `monitorFromPoint(x, y)` | 获取指定坐标处的显示器 | `Monitor \| null` |

```typescript
const monitors = app.monitors()
console.log(monitors)
// => [{ monitorId: 0, width: 1920, height: 1080, x: 0, y: 0, scaleFactor: 2, ... }]
```

### on(event, callback) / once(event, callback)

监听应用事件。返回取消监听的函数。

```typescript
const unsub = app.on('ready', () => console.log('就绪'))
app.once('quit', () => console.log('退出'))
unsub() // 取消监听
```

也可以监听窗口/托盘/菜单事件：

```typescript
app.on('main', 'close', () => console.log('main 窗口关闭'))
app.on('myTray', 'click', (data) => console.log('托盘点击', data))
```

应用事件：`ready`、`quit`

---

## BrowserWindow

浏览器窗口，包含 tao 窗口和 wry WebView 的绑定。

### 创建窗口

```typescript
import { BrowserWindow } from 'taowry'

// 必须在 Application 创建之后
const win = new BrowserWindow('label', {
  url: 'https://example.com',
  width: 800,
  height: 600,
  title: 'My Window',
})
```

第一个参数 `label` 是窗口唯一标识符，不可重复。
第二个参数为 [BrowserWindowAttributes](#browserwindowattributes) 配置对象。

### onCreated(callback)

窗口创建成功后的回调。

```typescript
win.onCreated((id) => {
  console.log('窗口创建成功, id:', id)
})
```

### 窗口事件

通过 `on` / `once` 监听窗口事件，返回取消监听的函数。

```typescript
win.on('move', (pos) => console.log('移动:', pos))
win.on('resize', (size) => console.log('缩放:', size))
win.on('focus', () => console.log('获得焦点'))
win.on('close', () => console.log('窗口关闭'))
```

快捷方法：

| 方法 | 事件 |
|------|------|
| `onMove(cb)` | `move` |
| `onClose(cb)` | `close` |
| `onDestroy(cb)` | `destroy` |
| `onBlur(cb)` | `blur` |
| `onFocus(cb)` | `focus` |
| `onCursorMove(cb)` | `cursorMove` |
| `onCursorEnter(cb)` | `cursorEnter` |
| `onCursorOut(cb)` | `cursorOut` |
| `onTheme(cb)` | `theme` |
| `onResize(cb)` | `resize` |

完整事件列表见 [WindowEvent](#windowevent)。

### RPC 通信

BrowserWindow 内置了 WebView 和 Node.js 之间的双向类型安全 RPC 通信。RPC 桥接脚本会在窗口创建时自动注入 WebView。

#### 定义 RPC 接口

```typescript
import { RPCInterface, RPCSchema, defineRPC, BrowserWindow } from 'taowry'

// 1. 定义双向 RPC 接口
interface MyRPC extends RPCInterface {
  host: RPCSchema<{
    requests: {
      getUserInfo: (data: { userId: string }) => { name: string }
    }
    messages: {
      pageReady: { url: string }
    }
  }>
  webview: RPCSchema<{
    requests: {
      renderData: (data: { items: string[] }) => { count: number }
    }
    messages: {
      userAction: { action: string }
    }
  }>
}

// 2. 创建 Host 端 RPC 配置（实现本端内容）
const rpc = defineRPC<MyRPC>({
  requests: {
    getUserInfo: async (data) => {
      const user = await db.findUser(data.userId)
      return { name: user.name }
    }
  },
  messages: {
    pageReady: (data) => console.log('页面就绪:', data.url)
  }
})

// 3. 创建窗口并绑定 RPC
const win = new BrowserWindow<MyRPC>('main', {
  url: 'https://example.com',
  rpc: rpc,
})
```

#### Host → WebView（调用 WebView 端方法）

```typescript
// 调用 WebView 端的 request 方法（request-response）
const result = await win.rpc.requests.renderData({ items: ['a', 'b'] })
console.log(result.count)

// 向 WebView 发送消息（fire-and-forget）
win.rpc.messages.userAction({ action: 'refresh' })
```

#### WebView → Host（WebView 端调用 Host）

WebView 端通过 `taowry/client` 子路径导入：

```typescript
import { defineRPC, RPCInterface, RPCSchema } from 'taowry/client'

interface MyRPC extends RPCInterface {
  host: RPCSchema<{
    requests: { getUserInfo: (data: { userId: string }) => { name: string } }
    messages: { pageReady: { url: string } }
  }>
  webview: RPCSchema<{
    requests: { renderData: (data: { items: string[] }) => { count: number } }
    messages: { userAction: { action: string } }
  }>
}

const rpc = defineRPC<MyRPC>({
  requests: {
    renderData: (data) => ({ count: data.items.length })
  },
  messages: {
    userAction: (data) => console.log('用户操作:', data.action)
  }
})

// 调用 Host 端方法
const user = await rpc.requests.getUserInfo({ userId: '123' })

// 监听 Host 发来的消息
rpc.on('pageReady', (data) => console.log('页面就绪:', data.url))

// 向 Host 发送消息
rpc.messages.pageReady({ url: location.href })
```

#### 动态注册/移除处理函数

```typescript
win.handle('getUser', async (data) => {
  return { name: 'Alice' }
})
win.removeHandler('getUser')
```

#### 向 WebView 发送消息

```typescript
// 触发 WebView 端 __taowry._handleSend，由 webview 端 on() 监听器接收
win.sendToWebview('update', { count: 42 })
```

#### WebView 端 IPC（原始消息）

```javascript
// WebView 向 Node 发送原始消息
window.ipc.postMessage('任意字符串')
```

Node 端通过 `ipcMessage` 事件接收：

```typescript
win.on('ipcMessage', (msg) => {
  console.log('收到 IPC:', msg.url, msg.body)
})
```

### views:// 自定义协议

WebView 端所有以 `views://` 开头的请求（页面加载、`fetch`、`<script src>`、`<img src>` 等）都会被拦截并转发到 Node 端处理。

#### 配置 protocol

通过 Application 统一配置协议响应逻辑（所有窗口共享）：

```typescript
import { Application } from 'taowry'

const app = new Application({
  protocol: async (request: Request): Promise<Response> => {
    const url = new URL(request.url)
    const path = url.pathname

    // 返回主页
    if (path === '/' || path === '/index.html') {
      return new Response('<html><body><h1>Hello</h1></body></html>', {
        status: 200,
        headers: { 'Content-Type': 'text/html' }
      })
    }

    // 返回 API 数据
    if (path === '/api/data') {
      return new Response(JSON.stringify({ message: 'Hello!' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' }
      })
    }

    // 404
    return new Response('Not Found', { status: 404 })
  }
})
```

> `protocol` 使用标准 Web API 的 `Request` / `Response`，与 `fetch` 风格一致。

#### 动态管理

```typescript
// 运行时更换 handler
app.setProtocol(async (request) => {
  return new Response('new handler', { headers: { 'Content-Type': 'text/plain' } })
})

// 移除 handler（后续请求自动返回 404）
app.removeProtocol()
```

#### URL 格式

`views://` 协议遵循标准 URL 格式：`views://host/path`

```typescript
// 加载主页
url: 'views://index.html'          // host=index.html, path=/
url: 'views://localhost/index.html' // host=localhost, path=/index.html

// 子资源请求
fetch('views://app/hello')          // host=app, path=/hello
<script src="views://app/bundle.js"> // host=app, path=/bundle.js
```

> Rust 端会透传 Node 返回的所有 headers，CORS 等策略由 handler 自行控制。

### WebView 操作

| 方法 | 说明 | 返回值 |
|------|------|--------|
| `close()` | 关闭窗口 | `void` |
| `requestRedraw()` | 请求重绘 | `void` |
| `setUrl(url)` | 设置 WebView URL | `void` |
| `loadUrlWithHeaders(url, headers)` | 带请求头加载 URL | `void` |
| `url()` | 获取当前 URL | `string` |
| `evaluateScript(script)` | 执行 JS（无返回值） | `void` |
| `evaluateScriptReturnResult(script)` | 执行 JS 并返回结果 | `Promise<string>` |
| `print()` | 打印页面 | `void` |
| `openDevtools()` | 打开开发者工具 | `void` |
| `closeDevtools()` | 关闭开发者工具 | `void` |
| `isDevtoolsOpen()` | 开发者工具是否打开 | `boolean` |
| `zoom(scale)` | 设置缩放比例 | `void` |
| `scaleFactor()` | 获取缩放因子 | `number` |
| `clearAllBrowsingData()` | 清除浏览数据 | `void` |
| `setBackgroundColor(color)` | 设置 WebView 背景色 `[r,g,b,a]` | `void` |
| `setWindowBackgroundColor(color)` | 设置窗口背景色 `[r,g,b,a]` 或 `null` | `void` |

```typescript
// 执行 JS
win.evaluateScript(`document.title = 'Hello'`)

// 执行 JS 并获取结果
const title = await win.evaluateScriptReturnResult(`document.title`)

// 带请求头加载
win.loadUrlWithHeaders('https://api.example.com', {
  Authorization: 'Bearer token'
})
```

### 窗口位置与尺寸

| 方法 | 说明 | 返回值 |
|------|------|--------|
| `position()` | 客户区域位置（不含边框/标题栏） | `Position` |
| `outerPosition()` | 窗口位置 | `Position` |
| `setPosition(x, y)` | 设置窗口位置 | `void` |
| `size()` | 客户区域尺寸 | `Size` |
| `setSize(width, height)` | 设置窗口尺寸 | `Size` |
| `outerSize()` | 整个窗口物理尺寸 | `Size` |
| `setMinSize(width, height)` | 设置最小尺寸 | `void` |
| `setMaxSize(width, height)` | 设置最大尺寸 | `void` |
| `setInnerSizeConstraints(c)` | 设置尺寸约束 | `void` |

```typescript
win.setSize(1280, 720)
win.setPosition(100, 100)
win.setMinSize(400, 300)
win.setMaxSize(1920, 1080)
```

### 窗口属性

| 方法 | 说明 | 返回值 |
|------|------|--------|
| `setTitle(title)` / `title()` | 设置/获取标题 | `void` / `string` |
| `setVisible(visible)` / `isVisible()` | 设置/获取可见性 | `void` / `boolean` |
| `setResizable(bool)` / `isResizable()` | 设置/获取可调整大小 | `void` / `boolean` |
| `setMinimizable(bool)` / `isMinimizable()` | 设置/获取可最小化 | `void` / `boolean` |
| `setMaximizable(bool)` / `isMaximizable()` | 设置/获取可最大化 | `void` / `boolean` |
| `setClosable(bool)` / `isClosable()` | 设置/获取可关闭 | `void` / `boolean` |
| `setEnabledButtons(buttons)` / `enabledButtons()` | 设置/获取控制按钮 | `void` / `WindowButton[]` |
| `minimized()` / `unminimized()` / `isMinimized()` | 最小化操作 | `void` / `boolean` |
| `maximized()` / `unmaximized()` / `isMaximized()` | 最大化操作 | `void` / `boolean` |

```typescript
win.setTitle('新标题')
const title = win.title()
win.setEnabledButtons(['close', 'minimize'])
win.minimized()
win.unminimized()
```

### 菜单

```typescript
// 设置窗口菜单栏
await win.setMenu([
  {
    text: '文件',
    items: [
      { text: '新建', accelerator: 'CmdOrCtrl+N' },
      { type: 'separator' },
      { text: '退出', type: 'predefined', item: 'quit' },
    ]
  },
])

// 设为应用全局菜单（仅 macOS）
win.setApplicationMenu()
```

详见 [Menu 菜单配置](#menu-菜单配置)。

### 全屏与装饰

| 方法 | 说明 |
|------|------|
| `fullscreen(monitorId?)` | 全屏。`null` 当前显示器全屏，传入 monitorId 在指定显示器全屏 |
| `unfullscreen()` | 取消全屏 |
| `isFullscreen()` | 获取全屏状态。`true` 当前全屏，`monitorId` 指定显示器全屏，`false` 未全屏 |
| `setDecorations(bool)` / `isDecorated()` | 设置/获取窗口装饰（标题栏、边框） |
| `borderless(bool)` / `isBorderless()` | 无边框窗口（`borderless()` 等同于 `setDecorations(false)`） |
| `setAlwaysOnTop(bool)` / `isAlwaysOnTop()` | 置顶 |
| `setAlwaysOnBottom(bool)` | 置底 |

> 无边框窗口可通过 CSS `-webkit-app-region: drag` 实现窗口拖动，
> `-webkit-app-region: no-drag` 排除不可拖动的交互区域（如按钮、输入框）。

### 外观与行为

| 方法 | 说明 |
|------|------|
| `setIcon(path)` | 设置窗口图标（Windows/X11） |
| `focus()` / `hasFocus()` | 聚焦窗口 / 获取焦点状态 |
| `setImePosition(pos)` | 设置输入法位置 |
| `setProgressBar(options)` | 设置任务栏进度条 |
| `requestUserAttention(type)` | 请求用户注意（闪烁任务栏/Dock 图标） |
| `cancelUserAttentionRequest()` | 取消注意力请求 |
| `setTheme(theme)` / `theme()` | 设置/获取主题（`'light'` / `'dark'` / `null` 跟随系统） |
| `setContentProtection(bool)` | 内容保护（防截屏） |
| `setVisibleOnAllWorkspaces(bool)` | 在所有工作区可见 |

```typescript
// 进度条
win.setProgressBar({ state: 'normal', progress: 50 })
win.setProgressBar(null) // 移除

// 请求用户注意
win.requestUserAttention('critical') // 闪烁直到获取焦点
win.requestUserAttention('informational') // 闪烁一次

// 主题
win.setTheme('dark')
win.setTheme('default') // 跟随系统
const theme = win.theme()
```

### 光标

| 方法 | 说明 |
|------|------|
| `setCursorIcon(cursor)` | 设置光标图标 |
| `setCursorPosition(pos)` | 设置光标位置 |
| `setCursorGrab(bool)` | 锁定/解锁光标 |
| `setCursorVisible(bool)` | 设置光标可见性 |
| `dragWindow()` | 拖拽窗口（需鼠标左键按下） |
| `dragResizeWindow(direction)` | 拖拽调整窗口大小（需鼠标左键按下，macOS 不支持） |
| `setIgnoreCursorEvents(bool)` | 忽略光标事件（窗口穿透） |
| `cursorPosition()` | 获取光标位置 |

---

## Tray

系统托盘图标。

```typescript
import { Tray } from 'taowry'

// 创建托盘图标（菜单直接传入数组）
const tray = new Tray('myTray', {
  icon: '/path/to/icon.png',
  tooltip: 'My App',
  title: 'MyApp',           // macOS 菜单栏标题
  menu: [                   // 菜单项数组
    { text: '显示窗口' },
    { text: '退出', type: 'predefined', item: 'quit' },
  ],
  iconAsTemplate: true,     // macOS 模板图标
  menuOnLeftClick: true,    // 左键单击显示菜单
})

// 监听托盘事件
tray.on('click', (data) => {
  console.log('托盘点击:', data)
})
tray.once('doubleClick', (data) => {
  // 双击打开窗口
  win.setVisible(true)
  win.focus()
})

// 动态操作
tray.setIcon('/path/to/new-icon.png')
tray.setTooltip('新提示')
tray.setTitle('新标题')    // macOS
tray.setVisible(false)
const rect = tray.rect()   // 获取图标区域
tray.remove()              // 移除托盘

// 动态设置菜单
await tray.setMenu([
  { text: '新项目' },
  { text: '退出', type: 'predefined', item: 'quit' },
])
```

托盘事件：`click`、`doubleClick`、`enter`、`move`、`leave`

---

## Menu 菜单配置

菜单配置直接传入 `MenuItemOptions[]` 数组，无需 `{ items: [...] }` 包裹。

### MenuItemOptions

| 属性 | 类型 | 说明 |
|------|------|------|
| `id` | `string` | 菜单项 ID（可选，自动生成） |
| `type` | `'normal' \| 'check' \| 'submenu' \| 'separator' \| 'predefined'` | 菜单项类型 |
| `text` | `string` | 显示文本 |
| `enabled` | `boolean` | 是否可用 |
| `checked` | `boolean` | 是否勾选（`check` 类型） |
| `accelerator` | `string` | 快捷键 |
| `item` | `PredefinedMenuItem` | 预定义项类型 |
| `items` | `MenuItemOptions[]` | 子菜单项（`submenu` 类型） |

**菜单项类型**：
- `normal` - 普通菜单项（默认）
- `check` - 复选菜单项（有 `checked` 时自动推断）
- `submenu` - 子菜单（有 `items` 时自动推断）
- `separator` - 分隔线
- `predefined` - 预定义项（配合 `item` 字段使用）

**预定义菜单项 (PredefinedMenuItem)**：
`separator`、`copy`、`cut`、`paste`、`selectAll`、`undo`、`redo`、`minimize`、`maximize`、`fullscreen`、`hide`、`hideOthers`、`showAll`、`closeWindow`、`quit`、`services`、`bringAllToFront`

**快捷键 (accelerator)**：
格式为 `Modifier+Key`，如 `CmdOrCtrl+S`、`CmdOrCtrl+Shift+Z`、`Alt+F4`。
Key 必须是标准键名（如 `N`、`S`、`=`、`-`），不支持 `Plus` 等别名。

```typescript
// 完整菜单示例
await app.setApplicationMenu([
  {
    text: '文件',
    items: [
      { text: '新建', accelerator: 'CmdOrCtrl+N' },
      { text: '打开', accelerator: 'CmdOrCtrl+O' },
      { type: 'separator' },
      { text: '退出', type: 'predefined', item: 'quit' },
    ]
  },
  {
    text: '编辑',
    items: [
      { type: 'predefined', item: 'undo' },
      { type: 'predefined', item: 'redo' },
      { type: 'separator' },
      { type: 'predefined', item: 'cut' },
      { type: 'predefined', item: 'copy' },
      { type: 'predefined', item: 'paste' },
      { type: 'predefined', item: 'selectAll' },
    ]
  },
  {
    text: '暗黑模式',
    type: 'check',
    checked: false,
  },
])
```

---

## getWindow

通过标签名获取已创建的窗口实例。

```typescript
import { getWindow, Window } from 'taowry'

const win = getWindow('main')
if (win) {
  win.focus()
}
```

> `Window` 是 `BrowserWindow` 的别名，可以直接使用。

---

## defineRPC

创建类型安全的 RPC 配置。`defineRPC` 的 config 始终实现**本端**内容：
- `requests`：实现对端调用的方法
- `messages`：实现对端发来的消息

```typescript
import { defineRPC } from 'taowry'       // Host 端
import { defineRPC } from 'taowry/client' // WebView 端
```

详见 [RPC 通信](#rpc-通信)。

---

## BrowserWindowAttributes

创建窗口时的完整配置项：

### WebView 配置

| 属性 | 类型 | 说明 |
|------|------|------|
| `url` | `string` | WebView 加载的 URL |
| `html` | `string` | WebView 加载的 HTML |
| `headers` | `Record<string, string>` | 请求头 |
| `backgroundColor` | `[r, g, b, a]` | WebView 背景色（0-255）。`transparent: true` 时未设置则默认透明 |
| `windowBackgroundColor` | `[r, g, b, a]` | 窗口背景色。`transparent: true` 时未设置则默认透明 |
| `devtools` | `boolean` | 启用开发者工具 |
| `userAgent` | `string` | 自定义 User-Agent |
| `clipboard` | `boolean` | 启用剪贴板 |
| `acceptFirstMouse` | `boolean` | 首次点击激活窗口（macOS） |
| `initializationScripts` | `string[]` | 页面加载前执行的脚本 |
| `navigationAllowed` | `boolean` | 是否允许导航（默认 `true`） |
| `newWindowAllowed` | `boolean` | 是否允许打开新窗口（默认 `true`） |
| `dragDropPreventDefault` | `boolean` | 阻止拖拽默认行为 |
| `downloadAllowed` | `boolean` | 是否允许下载（默认 `true`） |
| `downloadPath` | `string` | 下载目录 |
| `menu` | `MenuOptions` | 窗口菜单配置（菜单项数组） |
| `rpc` | `T['host']` | 窗口 RPC 配置 |

### 窗口配置

| 属性 | 类型 | 说明 |
|------|------|------|
| `width` | `number` | 窗口初始宽度（默认 800） |
| `height` | `number` | 窗口初始高度（默认 600） |
| `minWidth` | `number` | 最小窗口宽度 |
| `minHeight` | `number` | 最小窗口高度 |
| `maxWidth` | `number` | 最大窗口宽度 |
| `maxHeight` | `number` | 最大窗口高度 |
| `x` | `number` | 窗口初始 X 位置 |
| `y` | `number` | 窗口初始 Y 位置 |
| `resizable` | `boolean` | 是否可调整大小 |
| `minimizable` | `boolean` | 是否可最小化 |
| `maximizable` | `boolean` | 是否可最大化 |
| `closable` | `boolean` | 是否可关闭 |
| `enabledButtons` | `WindowButton[]` | 启用的控制按钮 |
| `title` | `string` | 窗口标题 |
| `maximized` | `boolean` | 是否最大化 |
| `visible` | `boolean` | 是否可见 |
| `transparent` | `boolean` | 是否透明 |
| `borderless` | `boolean` | 是否无边框。无边框窗口可通过 CSS `-webkit-app-region: drag` 拖动 |
| `decorations` | `boolean` | 是否显示装饰（标题栏、边框） |
| `titleBarStyle` | `TitleBarStyle` | 标题栏样式（macOS）：`'visible'` / `'hidden'` / `'hiddenInset'` |
| `trafficLightPosition` | `Position` | 交通灯按钮位置偏移（macOS，`titleBarStyle='hiddenInset'` 时有效） |
| `windowIcon` | `string` | 窗口图标路径 |
| `theme` | `Theme` | 窗口主题 |
| `contentProtected` | `boolean` | 内容保护（防截屏） |
| `visibleOnAllWorkspaces` | `boolean` | 所有工作区可见 |
| `active` | `boolean` | 是否激活窗口 |
| `focused` | `boolean` | 是否获取焦点 |
| `fullscreen` | `boolean \| Monitor['monitorId']` | 是否全屏 |
| `alwaysOnTop` | `boolean` | 是否置顶 |
| `alwaysOnBottom` | `boolean` | 是否置底 |

---

## WindowEvent

窗口事件类型映射：

| 事件 | 数据类型 | 说明 |
|------|----------|------|
| `created` | `WindowId` | 窗口创建完成 |
| `close` | `void` | 窗口被关闭 |
| `destroy` | `void` | 窗口被销毁 |
| `move` | `Position` | 窗口被移动 |
| `resize` | `Size` | 窗口大小变更 |
| `focus` | `void` | 获得焦点 |
| `blur` | `void` | 失去焦点 |
| `cursorMove` | `Position` | 鼠标在窗口上移动 |
| `cursorEnter` | `void` | 鼠标进入窗口 |
| `cursorOut` | `void` | 鼠标离开窗口 |
| `theme` | `Theme` | 主题变更 |
| `droppedFile` | `{ path: string }` | 文件被拖放到窗口 |
| `hoveredFile` | `{ path: string }` | 文件悬停在窗口上 |
| `hoveredFileCancelled` | `void` | 文件悬停取消 |
| `receivedImeText` | `string` | 收到输入法文本 |
| `keyboardInput` | `any` | 键盘输入 |
| `modifiersChanged` | `{ shift, control, alt, super }` | 修饰键变化 |
| `mouseWheel` | `any` | 鼠标滚轮 |
| `mouseInput` | `any` | 鼠标按键 |
| `touchpadPressure` | `{ pressure, stage }` | 触控板压力 |
| `axisMotion` | `{ axis, value }` | 轴运动 |
| `touch` | `any` | 触摸事件 |
| `scaleFactorChanged` | `{ scaleFactor, innerSize }` | 缩放因子变化 |
| `decorationsClick` | `void` | 装饰区域点击 |
| `ipcMessage` | `{ url, body }` | WebView IPC 消息 |
| `navigation` | `{ url }` | 导航事件 |
| `newWindow` | `{ url }` | 新窗口请求 |
| `documentTitleChanged` | `{ title }` | 文档标题变化 |
| `pageLoad` | `{ event, url }` | 页面加载（started/finished） |
| `dragDrop` | `any` | 拖放事件 |
| `downloadStarted` | `{ url, path }` | 下载开始 |
| `downloadCompleted` | `{ url, path, success }` | 下载完成 |

> 响应操作系统发出的事件，使用 API 变更窗口状态不会触发对应事件。

---

## 类型定义

```typescript
type WindowId = string

type Size = { width: number; height: number }

type Position = { x: number; y: number }

type Rect = Position & Size

type Monitor = Size & Position & {
  monitorId: number
  name?: string | null
  scaleFactor: number
}

type WindowButton = 'close' | 'minimize' | 'maximize'
type TitleBarStyle = 'visible' | 'hidden' | 'hiddenInset'
type Theme = 'light' | 'dark'
type UserAttentionType = 'critical' | 'informational'
type ResizeDirection = 'east' | 'north' | 'northEast' | 'northWest'
                     | 'south' | 'southEast' | 'southWest' | 'west'
type ProgressState = 'none' | 'normal' | 'indeterminate' | 'paused' | 'error'

interface ProgressBarOptions {
  state?: ProgressState
  progress?: number
  desktopFilename?: string
}

interface WindowSizeConstraints {
  minWidth?: number
  minHeight?: number
  maxWidth?: number
  maxHeight?: number
}

interface ProtocolHandler {
  (request: Request): Response | Promise<Response>
}

interface ApplicationOptions {
  protocol?: ProtocolHandler
}

interface TrayIconOptions {
  icon?: string              // 图标路径
  tooltip?: string           // 鼠标悬停提示
  title?: string             // 标题（macOS）
  menu?: MenuOptions         // 菜单项数组
  tempDirPath?: string       // 临时目录路径
  iconAsTemplate?: boolean   // 模板图标（macOS）
  menuOnLeftClick?: boolean  // 左键单击显示菜单
}

/** 菜单配置 (即菜单项数组) */
type MenuOptions = MenuItemOptions[]

interface MenuItemOptions {
  id?: string
  type?: 'normal' | 'check' | 'submenu' | 'separator' | 'predefined'
  text?: string
  enabled?: boolean
  checked?: boolean
  accelerator?: string
  item?: PredefinedMenuItem
  items?: MenuItemOptions[]
}
```

---

## 注意事项

1. **架构**：基于 napi-rs 构建的原生模块，TS 直接同步调用 Rust napi 函数，无中间层。Application 构造时启动 Rust 事件循环线程，窗口/菜单/托盘等操作均为同步调用。

2. **RPC 通信架构**：RPC 协议由 Rust 层解析和路由（基于 wry 原生 `with_ipc_handler` + `evaluate_script`），Node.js 侧为薄消费者。Host→WebView 请求采用延迟响应模式——Rust 分配请求 ID 并追踪映射，WebView 响应后才将结果发回 Node.js。非 RPC 的 `window.ipc.postMessage()` 仍通过 `ipcMessage` 事件透传。

3. **菜单快捷键**：快捷键格式为 `Modifier+Key`，其中 Key 必须是标准键名（如 `N`、`S`、`=`、`-`），不支持 `Plus` 等别名。

4. **evaluateScriptReturnResult**：在 `file://` URL 下 wry 存在已知问题，建议使用 `http://` URL 加载页面，或使用 `evaluateScript` 配合 RPC 通信获取返回值。

5. **应用菜单**：`setApplicationMenu` 仅在 macOS 上有效。Windows/Linux 请使用 `setWindowMenu` 或 `win.setMenu()` 设置窗口菜单。

6. **透明窗口**：创建窗口时设置 `transparent: true`，WebView 和窗口背景色会自动设为透明。

7. **无边框窗口拖动**：wry 0.55+ 原生支持 CSS `-webkit-app-region: drag` 拖动窗口，`-webkit-app-region: no-drag` 排除交互区域（按钮、输入框等），无需额外代码。

8. **本地开发**：运行 `bun run build` 编译 Rust 原生模块（build.sh 会自动同步到 `src/ts/` 和 `binary/`），然后通过 `bun start` 运行测试。修改 Rust 代码后必须重新编译。

9. **views:// 协议**：WebView 端所有 `views://` 请求通过 Rust → Node IPC 中转处理。请求体和响应体使用 base64 编码传输，对于常规 HTML/JS/CSS 内容（KB 到几百 KB）无性能问题，大型资源（MB 级别）可能有延迟。

10. **macOS 平台依赖**：macOS 平台特定功能（Dock、事件排空）使用 `objc2` + `objc2-foundation` crate 实现原生 Objective-C 互操作，已替代旧的 `cocoa`/`objc` crate。
