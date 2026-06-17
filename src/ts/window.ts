import { readFileSync } from 'fs'
import type {
  BrowserWindowAttributes,
  CursorIcon,
  MenuOptions,
  Monitor,
  Position,
  ProgressBarOptions,
  ResizeDirection,
  Size,
  Theme,
  UserAttentionType,
  WindowButton,
  WindowEvent,
  WindowId,
  WindowSizeConstraints,
  RPCInterface,
  HostRPCInstance,
  RPCPromise
} from './types'
import { getCurrentApplication } from './app'
import { Menu } from './menu'
import { native, json, parse } from './native-module'

/** 注入到 WebView 的 RPC 桥接脚本 */
const RPC_BRIDGE_SCRIPT = `
(function() {
  if (window.__taowry) return;
  var counter = 0;
  var callbacks = {};
  var rpcHandlers = {};
  var messageHandlers = {};

  window.__taowry = {
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
              window.ipc.postMessage(JSON.stringify({ type: "req", id: id, method: method, data: data }));
            });
          };
        }
      });
      var rpcMessages = new Proxy({}, {
        get: function(_, event) {
          return function(data) {
            window.ipc.postMessage(JSON.stringify({ type: "msg", event: event, data: data }));
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
    _resolve: function(id, data, error) {
      var cb = callbacks[id];
      if (!cb) return;
      delete callbacks[id];
      if (error) cb.reject(new Error(error));
      else cb.resolve(data);
    },
    _handleInvoke: function(payload) {
      var id = payload.id;
      var method = payload.method;
      var data = payload.data;
      var handler = rpcHandlers[method];
      if (!handler) {
        window.ipc.postMessage(JSON.stringify({ type: "res", id: id, error: "No handler for: " + method }));
        return;
      }
      try {
        var result = handler(data);
        if (result && typeof result.then === "function") {
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
    _handleSend: function(event, data) {
      setTimeout(function() {
        var mh = messageHandlers[event] || [];
        mh.forEach(function(cb) { cb(data); });
      }, 0);
    }
  };
})();
`

type RpcHandler = (data: any) => any | Promise<any>

/**
 * BrowserWindow - 浏览器窗口
 *
 * 包含 tao 窗口和 wry WebView 的绑定，提供窗口管理、WebView 操作、RPC 通信等功能。
 * 必须在 Application 创建之后才能实例化。
 *
 * @template T - RPC 接口类型定义
 */
export default class BrowserWindow<T extends RPCInterface = any> {
  /** 窗口唯一标识符 */
  readonly label: string
  /** 窗口原生 ID */
  readonly id: WindowId
  private _autoMenu?: Menu
  private _rpc?: HostRPCInstance<T>
  private rpcHandlers: Map<string, RpcHandler> = new Map()
  private _rpcMessageListeners: Record<string, Function[]> = {}

  /**
   * 创建浏览器窗口
   * @param label - 窗口唯一标识符，不可重复
   * @param props - 窗口配置项 @see BrowserWindowAttributes
   */
  constructor(label: string, props: BrowserWindowAttributes<T> = {}) {
    const app = getCurrentApplication()
    if (!app) {
      throw new Error('Create an Application before creating BrowserWindow')
    }
    this.label = label
    app._registerWindow(this)

    // 注入 RPC 桥接脚本
    if (!props.initializationScripts) props.initializationScripts = []
    props.initializationScripts.unshift(RPC_BRIDGE_SCRIPT)

    // 监听 Rust 转发的 WebView→Host RPC 请求
    this.on('rpcRequest' as any, (msg: { rpcId: number; method: string; data: any }) => {
      const handler = this.rpcHandlers.get(msg.method)
      if (!handler) {
        app._rpcResolve(this.label, msg.rpcId, null, `RPC method '${msg.method}' is not registered`)
        return
      }
      Promise.resolve()
        .then(() => handler(msg.data))
        .then(result => app._rpcResolve(this.label, msg.rpcId, result))
        .catch(err => app._rpcResolve(this.label, msg.rpcId, null, err?.message || String(err)))
    })

    // 监听 WebView→Host 单向消息
    this.on('rpcMessage' as any, (msg: { event: string; data: any }) => {
      const listeners = (this._rpcMessageListeners[msg.event] || []).slice()
      queueMicrotask(() => {
        for (const cb of listeners) {
          try {
            cb(msg.data)
          } catch (e) {
            console.error(`RPC message listener error [${msg.event}]:`, e)
          }
        }
      })
    })

    // 处理 props.rpc
    if (props.rpc) {
      this._rpc = this.createRPC(props.rpc)
    }

    // 清除不可序列化的字段
    const createProps: any = { ...props }
    delete createProps.rpc
    delete createProps.menu

    // 处理 props.menu
    const menuConfig = props.menu
    if (menuConfig) {
      const menuLabel = `${label}:auto-menu`
      this._autoMenu = new Menu(menuLabel, menuConfig)
    }

    // assets:// 协议：自动注入内部 host，确保 URL 结构完整
    // assets://index.html → assets://__taowry__/index.html
    if (createProps.url?.startsWith('assets://')) {
      createProps.url = createProps.url.replace('assets://', 'assets://__taowry__/')
    }
    // 读取窗口图标，转化为base64传入以支持虚拟路径
    if (createProps.windowIcon) {
      createProps.windowIcon = readFileSync(createProps.windowIcon).toBase64()
    }
    // 同步创建窗口
    this.id = native.createWindow(label, json(createProps))
    this.app._emit(this.label, 'created', this.id)

    // 自动关联菜单
    if (this._autoMenu) {
      this._autoMenu.created.then(() => {
        native.setWindowMenu(label, this._autoMenu!.label)
      })
    }
  }

  /** RPC 通信实例，用于 Host↔WebView 双向通信 */
  get rpc(): HostRPCInstance<T> | undefined {
    return this._rpc
  }

  // ===== 事件 =====

  /** 监听窗口事件，返回取消监听的函数 */
  on<E extends keyof WindowEvent>(event: E, callback: (data: WindowEvent[E]) => void) {
    return this.app.on(this.label, event, callback as any)
  }

  /** 监听窗口事件（仅触发一次），返回取消监听的函数 */
  once<E extends keyof WindowEvent>(event: E, callback: (data: WindowEvent[E]) => void) {
    return this.app.once(this.label, event, callback as any)
  }

  /** 监听窗口创建完成 */
  onCreated(callback: (id: WindowId) => void) {
    return this.on('created', callback)
  }
  /** 监听窗口移动 */
  onMove(callback: (data: Position) => void) {
    return this.on('move', callback)
  }
  /** 监听窗口关闭 */
  onClose(callback: () => void) {
    return this.on('close', callback)
  }
  /** 监听窗口销毁 */
  onDestroy(callback: () => void) {
    return this.on('destroy', callback)
  }
  /** 监听窗口失去焦点 */
  onBlur(callback: () => void) {
    return this.on('blur', callback)
  }
  /** 监听窗口获得焦点 */
  onFocus(callback: () => void) {
    return this.on('focus', callback)
  }
  /** 监听鼠标在窗口上移动 */
  onCursorMove(callback: (data: Position) => void) {
    return this.on('cursorMove', callback)
  }
  /** 监听鼠标进入窗口 */
  onCursorEnter(callback: () => void) {
    return this.on('cursorEnter', callback)
  }
  /** 监听鼠标离开窗口 */
  onCursorOut(callback: () => void) {
    return this.on('cursorOut', callback)
  }
  /** 监听主题变更 */
  onTheme(callback: (data: Theme) => void) {
    return this.on('theme', callback)
  }
  /** 监听窗口大小变更 */
  onResize(callback: (data: Size) => void) {
    return this.on('resize', callback)
  }

  // ===== 菜单 =====

  /** 设置窗口菜单栏 */
  async setMenu(menu: MenuOptions | string): Promise<void> {
    if (typeof menu === 'string') {
      native.setWindowMenu(this.label, menu)
      return
    }
    const menuLabel = `${this.label}:set-menu`
    const autoMenu = new Menu(menuLabel, menu)
    await autoMenu.created
    native.setWindowMenu(this.label, menuLabel)
  }

  /** 将此窗口菜单设为应用全局菜单（仅 macOS 有效） */
  setApplicationMenu(): void {
    if (this._autoMenu) {
      native.setApplicationMenu(this._autoMenu.label)
    } else {
      native.setApplicationMenu(`${this.label}:auto-menu`)
    }
  }

  // ===== RPC =====

  private createRPC(config: BrowserWindowAttributes<T>['rpc']): HostRPCInstance<T> {
    if (!config) throw new Error('RPC config is required')
    if (config.requests) {
      for (const [method, handler] of Object.entries(config.requests)) {
        if (handler) this.rpcHandlers.set(method, handler as RpcHandler)
      }
    }
    if (config.messages) {
      for (const [event, handler] of Object.entries(config.messages)) {
        if (handler) {
          if (!this._rpcMessageListeners[event]) this._rpcMessageListeners[event] = []
          this._rpcMessageListeners[event].push(handler as Function)
        }
      }
    }
    const self = this
    const requestsProxy = new Proxy({} as any, {
      get(_, method: string) {
        return (data: any) => self.app._rpcInvoke(self.label, method, data)
      }
    })
    const messagesProxy = new Proxy({} as any, {
      get(_, event: string) {
        return (data: any) => self.app._rpcSend(self.label, event, data)
      }
    })
    return {
      requests: requestsProxy,
      messages: messagesProxy,
      on: (event: keyof RPCPromise<T['host'], 'messages'>, callback: (...args: any[]) => void) => {
        const key = event as string
        if (!this._rpcMessageListeners[key]) this._rpcMessageListeners[key] = []
        this._rpcMessageListeners[key].push(callback as Function)
        return () => {
          this._rpcMessageListeners[key] = this._rpcMessageListeners[key].filter(
            cb => cb !== (callback as Function)
          )
        }
      },
      off: (event: keyof RPCPromise<T['host'], 'messages'>, callback: (...args: any[]) => void) => {
        const key = event as string
        if (!this._rpcMessageListeners[key]) return
        this._rpcMessageListeners[key] = this._rpcMessageListeners[key].filter(
          cb => cb !== (callback as Function)
        )
      }
    } as HostRPCInstance<T>
  }

  /** 动态注册 RPC 处理函数（WebView→Host 方向） */
  handle(method: string, handler: RpcHandler) {
    this.rpcHandlers.set(method, handler)
  }
  /** 移除已注册的 RPC 处理函数 */
  removeHandler(method: string) {
    this.rpcHandlers.delete(method)
  }

  /**
   * 向 WebView 发送消息（fire-and-forget）
   * 触发 WebView 端 `__taowry.on(event, callback)` 监听器
   */
  sendToWebview(event: string, data?: any): void {
    const payload = json(data)
    native.windowEvaluateScript(
      this.label,
      `window.__taowry && window.__taowry._handleSend(${json(event)}, ${payload})`
    )
  }

  // ===== WebView 操作 =====

  /** 关闭窗口 */
  close(): void {
    native.windowClose(this.label)
  }
  /** 请求重绘 */
  requestRedraw(): void {
    native.windowRequestRedraw(this.label)
  }
  /** 设置 WebView URL */
  setUrl(url: string): void {
    if (url.startsWith('assets://')) url = url.replace('assets://', 'assets://__taowry__/')
    native.windowSetUrl(this.label, url)
  }
  /** 带请求头加载 URL */
  loadUrlWithHeaders(url: string, headers: Record<string, string>): void {
    if (url.startsWith('assets://')) url = url.replace('assets://', 'assets://__taowry__/')
    native.windowLoadUrlWithHeaders(this.label, json({ url, headers }))
  }
  /** 获取当前 WebView URL */
  url(): string {
    return native.windowUrl(this.label)
  }
  /** 执行 JS 脚本（无返回值） */
  evaluateScript(script: string): void {
    native.windowEvaluateScript(this.label, script)
  }
  /** 执行 JS 脚本并返回结果 */
  evaluateScriptReturnResult(script: string): Promise<string> {
    return this.app._evaluateScript(this.label, script)
  }
  /** 打印页面 */
  print(): void {
    native.windowPrint(this.label)
  }
  /** 打开开发者工具 */
  openDevtools(): void {
    native.windowOpenDevtools(this.label)
  }
  /** 关闭开发者工具 */
  closeDevtools(): void {
    native.windowCloseDevtools(this.label)
  }
  /** 开发者工具是否打开 */
  isDevtoolsOpen(): boolean {
    return native.windowIsDevtoolsOpen(this.label)
  }
  /** 设置 WebView 缩放比例 */
  zoom(scale: number): void {
    native.windowZoom(this.label, scale)
  }
  /** 获取窗口缩放因子 */
  scaleFactor(): number {
    return native.windowScaleFactor(this.label)
  }
  /** 清除所有浏览数据 */
  clearAllBrowsingData(): void {
    native.windowClearAllBrowsingData(this.label)
  }
  /** 设置 WebView 背景色 [r, g, b, a] (0-255) */
  setBackgroundColor(color: [number, number, number, number]): void {
    native.windowSetBackgroundColor(this.label, json(color))
  }
  /** 设置窗口背景色 [r, g, b, a] (0-255)，传 null 恢复默认 */
  setWindowBackgroundColor(color: [number, number, number, number] | null): void {
    native.windowSetWindowBackgroundColor(this.label, json(color))
  }

  // ===== 窗口位置/尺寸 =====

  /** 获取客户区域位置（不含边框/标题栏） */
  position(): Position {
    return parse(native.windowInnerPosition(this.label))
  }
  /** 获取窗口位置（含边框/标题栏） */
  outerPosition(): Position {
    return parse(native.windowOuterPosition(this.label))
  }
  /** 设置窗口位置 */
  setPosition(x: number, y: number): void {
    native.windowSetOuterPosition(this.label, json({ x, y }))
  }
  /** 获取客户区域尺寸 */
  size(): Size {
    return parse(native.windowInnerSize(this.label))
  }
  /** 设置客户区域尺寸，返回实际尺寸 */
  setSize(width: number, height: number): Size {
    return parse(native.windowSetInnerSize(this.label, json({ width, height })))
  }
  /** 获取整个窗口物理尺寸 */
  outerSize(): Size {
    return parse(native.windowOuterSize(this.label))
  }
  /** 设置最小尺寸 */
  setMinSize(width: number, height: number): void {
    native.windowSetMinInnerSize(this.label, json({ width, height }))
  }
  /** 设置最大尺寸 */
  setMaxSize(width: number, height: number): void {
    native.windowSetMaxInnerSize(this.label, json({ width, height }))
  }
  /** 设置尺寸约束 */
  setInnerSizeConstraints(constraints: WindowSizeConstraints): void {
    native.windowSetInnerSizeConstraints(this.label, json(constraints))
  }

  // ===== 窗口属性 =====

  /** 设置窗口标题 */
  setTitle(title: string): void {
    native.windowSetTitle(this.label, title)
  }
  /** 获取窗口标题 */
  title(): string {
    return native.windowTitle(this.label)
  }
  /** 设置窗口可见性 */
  setVisible(visible: boolean): void {
    native.windowSetVisible(this.label, visible)
  }
  /** 获取窗口可见性 */
  isVisible(): boolean {
    return native.windowIsVisible(this.label)
  }
  /** 设置窗口是否可调整大小 */
  setResizable(resizable: boolean): void {
    native.windowSetResizable(this.label, resizable)
  }
  /** 获取窗口是否可调整大小 */
  isResizable(): boolean {
    return native.windowIsResizable(this.label)
  }
  /** 设置窗口是否可最小化 */
  setMinimizable(minimizable: boolean): void {
    native.windowSetMinimizable(this.label, minimizable)
  }
  /** 获取窗口是否可最小化 */
  isMinimizable(): boolean {
    return native.windowIsMinimizable(this.label)
  }
  /** 设置窗口是否可最大化 */
  setMaximizable(maximizable: boolean): void {
    native.windowSetMaximizable(this.label, maximizable)
  }
  /** 获取窗口是否可最大化 */
  isMaximizable(): boolean {
    return native.windowIsMaximizable(this.label)
  }
  /** 设置窗口是否可关闭 */
  setClosable(closable: boolean): void {
    native.windowSetClosable(this.label, closable)
  }
  /** 获取窗口是否可关闭 */
  isClosable(): boolean {
    return native.windowIsClosable(this.label)
  }
  /** 设置启用的控制按钮 */
  setEnabledButtons(buttons: WindowButton[] = ['close', 'maximize', 'minimize']): void {
    native.windowSetEnabledButtons(this.label, json(buttons))
  }
  /** 获取启用的控制按钮 */
  enabledButtons(): WindowButton[] {
    return parse(native.windowEnabledButtons(this.label))
  }
  /** 最小化窗口 */
  minimized(): void {
    native.windowSetMinimized(this.label, true)
  }
  /** 取消最小化 */
  unminimized(): void {
    native.windowSetMinimized(this.label, false)
  }
  /** 获取窗口是否最小化 */
  isMinimized(): boolean {
    return native.windowIsMinimized(this.label)
  }
  /** 最大化窗口 */
  maximized(): void {
    native.windowSetMaximized(this.label, true)
  }
  /** 取消最大化 */
  unmaximized(): void {
    native.windowSetMaximized(this.label, false)
  }
  /** 获取窗口是否最大化 */
  isMaximized(): boolean {
    return native.windowIsMaximized(this.label)
  }

  // ===== 全屏/装饰/层级 =====

  /**
   * 进入全屏
   * @param monitorId - null 当前显示器全屏，传入 monitorId 在指定显示器全屏
   */
  fullscreen(monitorId?: null | Monitor['monitorId']): void {
    native.windowFullscreen(this.label, json(monitorId ?? null))
  }
  /** 退出全屏 */
  unfullscreen(): void {
    native.windowUnfullscreen(this.label)
  }
  /**
   * 获取全屏状态
   * @returns true=当前全屏，monitorId=指定显示器全屏，false=未全屏
   */
  isFullscreen(): Monitor['monitorId'] | boolean {
    const result = native.windowIsFullscreen(this.label)
    return result === 'true' ? true : result === 'false' ? false : parse<number>(result)
  }
  /** 设置窗口装饰（标题栏、边框） */
  setDecorations(decorations: boolean): void {
    native.windowSetDecorations(this.label, decorations)
  }
  /** 获取窗口是否有装饰 */
  isDecorated(): boolean {
    return native.windowIsDecorated(this.label)
  }
  /** 设置无边框（等同于 setDecorations(!borderless)） */
  borderless(borderless = true): void {
    this.setDecorations(!borderless)
  }
  /** 获取是否无边框 */
  isBorderless(): boolean {
    return !this.isDecorated()
  }
  /** 设置置顶 */
  setAlwaysOnTop(top = true): void {
    native.windowSetAlwaysOnTop(this.label, top)
  }
  /** 获取是否置顶 */
  isAlwaysOnTop(): boolean {
    return native.windowIsAlwaysOnTop(this.label)
  }
  /** 设置置底 */
  setAlwaysOnBottom(bottom = true): void {
    native.windowSetAlwaysOnBottom(this.label, bottom)
  }

  // ===== 外观/行为 =====

  /** 设置窗口图标 (Windows/X11) */
  setIcon(icon: string): void {
    native.windowSetWindowIcon(this.label, readFileSync(icon))
  }
  /** 聚焦窗口 */
  focus(): void {
    native.windowFocus(this.label)
  }
  /** 获取窗口是否有焦点 */
  hasFocus(): boolean {
    return native.windowHasFocus(this.label)
  }
  /** 设置输入法位置 */
  setImePosition(position: Position): void {
    native.windowSetImePosition(this.label, json(position))
  }
  /** 设置任务栏进度条，传 null 移除 */
  setProgressBar(progress: ProgressBarOptions | null): void {
    native.windowSetProgressBar(this.label, json(progress))
  }
  /** 请求用户注意（闪烁任务栏/Dock 图标） */
  requestUserAttention(type: UserAttentionType = 'critical'): void {
    native.windowRequestUserAttention(this.label, json(type))
  }
  /** 取消注意力请求 */
  cancelUserAttentionRequest(): void {
    native.windowRequestUserAttention(this.label, 'null')
  }
  /**
   * 设置窗口主题
   * @param theme - 'light' | 'dark' | null 跟随系统 | 'default' 跟随系统
   */
  setTheme(theme: Theme | 'default' | null): void {
    native.windowSetTheme(this.label, json(theme === 'default' ? null : theme))
  }
  /** 获取窗口主题 */
  theme(): Theme | null {
    return native.windowTheme(this.label) as Theme
  }
  /** 设置内容保护（防截屏） */
  setContentProtection(enabled: boolean): void {
    native.windowSetContentProtection(this.label, enabled)
  }
  /** 设置在所有工作区可见 */
  setVisibleOnAllWorkspaces(visible: boolean): void {
    native.windowSetVisibleOnAllWorkspaces(this.label, visible)
  }

  // ===== 光标 =====

  /** 设置光标图标 */
  setCursorIcon(cursor: CursorIcon): void {
    native.windowSetCursorIcon(this.label, cursor)
  }
  /** 设置光标位置 */
  setCursorPosition(position: Position): void {
    native.windowSetCursorPosition(this.label, json(position))
  }
  /** 锁定/解锁光标 */
  setCursorGrab(grab: boolean): void {
    native.windowSetCursorGrab(this.label, grab)
  }
  /** 设置光标可见性 */
  setCursorVisible(visible: boolean): void {
    native.windowSetCursorVisible(this.label, visible)
  }
  /** 拖拽窗口（需鼠标左键按下） */
  dragWindow(): void {
    native.windowDragWindow(this.label)
  }
  /** 拖拽调整窗口大小（需鼠标左键按下，macOS 不支持） */
  dragResizeWindow(direction: ResizeDirection): void {
    native.windowDragResizeWindow(this.label, direction)
  }
  /** 忽略光标事件（窗口穿透） */
  setIgnoreCursorEvents(ignore: boolean): void {
    native.windowSetIgnoreCursorEvents(this.label, ignore)
  }
  /** 获取光标位置 */
  cursorPosition(): Position {
    return parse(native.windowCursorPosition(this.label))
  }

  private get app() {
    const app = getCurrentApplication()
    if (!app) throw new Error('Application has not been created')
    return app
  }
}
