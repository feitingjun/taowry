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

/** 注入到 WebView 的 RPC 桥接脚本 */
const RPC_BRIDGE_SCRIPT = `
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

    // 内部：处理 Host→WebView 的消息（setTimeout 保留 debugger 安全修复）
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

export default class BrowserWindow<T extends RPCInterface = any> {
  readonly label: string
  private _id?: WindowId
  private _created: Promise<WindowId>
  private _autoMenu?: Menu
  private _rpc?: HostRPCInstance<T>
  private rpcHandlers: Map<string, RpcHandler> = new Map()
  private _rpcMessageListeners: Record<string, Function[]> = {}

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
        this.send('rpc_resolve', {
          id: msg.rpcId,
          data: null,
          error: `RPC method '${msg.method}' is not registered`
        })
        return
      }
      Promise.resolve()
        .then(() => handler(msg.data))
        .then(result => this.send('rpc_resolve', { id: msg.rpcId, data: result }))
        .catch(err =>
          this.send('rpc_resolve', { id: msg.rpcId, data: null, error: err?.message || String(err) })
        )
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

    // 处理 props.rpc：创建 RPC 实例
    if (props.rpc) {
      this._rpc = this.createRPC(props.rpc)
    }

    // 处理 props.menu：自动创建菜单并关联到窗口
    const menuConfig = props.menu
    if (menuConfig) {
      const menuLabel = `${label}:auto-menu`
      this._autoMenu = new Menu(menuLabel, menuConfig)
    }

    this._created = this.create(props)

    // 菜单创建完成后自动关联到窗口
    if (this._autoMenu) {
      const autoMenu = this._autoMenu
      this._created = this._created.then(async id => {
        await autoMenu.created
        await this.setMenu(autoMenu.label)
        return id
      })
    }
  }

  /** 获取窗口唯一 ID (由 Rust 端分配) */
  get id(): WindowId | undefined {
    return this._id
  }

  /** 窗口创建完成的 Promise */
  get created(): Promise<WindowId> {
    return this._created
  }

  /** 获取窗口的 RPC 实例（由 props.rpc 创建） */
  get rpc(): HostRPCInstance<T> | undefined {
    return this._rpc
  }

  // ===== 事件 =====

  on<T extends keyof WindowEvent>(event: T, callback: (data: WindowEvent[T]) => void) {
    return this.app.on(this.label, event, callback as any)
  }

  once<T extends keyof WindowEvent>(event: T, callback: (data: WindowEvent[T]) => void) {
    return this.app.once(this.label, event, callback as any)
  }

  onCreated(callback: (id: WindowId) => void) {
    return this.on('created', callback)
  }

  onMove(callback: (data: Position) => void) {
    return this.on('move', callback)
  }
  onClose(callback: () => void) {
    return this.on('close', callback)
  }
  onDestroy(callback: () => void) {
    return this.on('destroy', callback)
  }
  onBlur(callback: () => void) {
    return this.on('blur', callback)
  }
  onFocus(callback: () => void) {
    return this.on('focus', callback)
  }
  onCursorMove(callback: (data: Position) => void) {
    return this.on('cursorMove', callback)
  }
  onCursorEnter(callback: () => void) {
    return this.on('cursorEnter', callback)
  }
  onCursorOut(callback: () => void) {
    return this.on('cursorOut', callback)
  }
  onTheme(callback: (data: Theme) => void) {
    return this.on('theme', callback)
  }
  onResize(callback: (data: Size) => void) {
    return this.on('resize', callback)
  }

  // ===== 菜单 =====

  /** 设置此窗口的菜单栏 */
  async setMenu(menu: MenuOptions | string): Promise<void> {
    if (typeof menu === 'string') {
      return this.send('set_window_menu', { window: this.label, menu })
    }
    const menuLabel = `${this.label}:set-menu`
    const autoMenu = new Menu(menuLabel, menu)
    await autoMenu.created
    return this.send('set_window_menu', { window: this.label, menu: menuLabel })
  }

  /** 将此窗口的菜单设为应用全局菜单 (仅 macOS) */
  setApplicationMenu(): Promise<void> {
    if (this._autoMenu) {
      return this.app.setApplicationMenu(this._autoMenu.items)
    }
    return this.send('set_application_menu', { menu: `${this.label}:auto-menu` })
  }

  // ===== RPC: createRPC =====

  /**
   * 创建 Host 端 RPC 实例（由 props.rpc 触发）
   * 支持 Host ↔ WebView 双向通信，包含 request-response 和 messages (fire-and-forget) 两种模式
   */
  private createRPC(config: BrowserWindowAttributes<T>['rpc']): HostRPCInstance<T> {
    if (!config) throw new Error('RPC config is required')

    // 注册当前侧的 request handlers（对端调用我）
    if (config.requests) {
      for (const [method, handler] of Object.entries(config.requests)) {
        if (handler) this.rpcHandlers.set(method, handler as RpcHandler)
      }
    }

    // 注册 messages 监听
    if (config.messages) {
      for (const [event, handler] of Object.entries(config.messages)) {
        if (handler) {
          if (!this._rpcMessageListeners[event]) this._rpcMessageListeners[event] = []
          this._rpcMessageListeners[event].push(handler as Function)
        }
      }
    }

    // 构建 requests 代理（调用 WebView 端方法，通过 Rust rpc_invoke 延迟响应）
    const self = this
    const requestsProxy = new Proxy({} as any, {
      get(_, method: string) {
        return (data: any) => self.send('rpc_invoke', { method, data })
      }
    })

    // 构建 messages 代理（向 WebView 端发送消息，通过 Rust rpc_send）
    const messagesProxy = new Proxy({} as any, {
      get(_, event: string) {
        return (data: any) => self.send('rpc_send', { event, data })
      }
    })

    // 返回 RPC 实例
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

  // ===== RPC: WebView → Node =====

  /**
   * 注册 RPC 处理函数
   * WebView 端通过 rpc.requests.method(data) 调用
   * @param method 方法名
   * @param handler 处理函数，返回值会传回 WebView
   */
  handle(method: string, handler: RpcHandler) {
    this.rpcHandlers.set(method, handler)
  }

  /** 移除 RPC 处理函数 */
  removeHandler(method: string) {
    this.rpcHandlers.delete(method)
  }

  // ===== RPC: Node → WebView =====

  /**
   * 向 WebView 发送消息（fire-and-forget）
   * WebView 端通过 rpc.on(event, callback) 监听
   */
  sendToWebview(event: string, data?: any): void {
    const payload = JSON.stringify(data === undefined ? null : data)
    this.evaluateScript(
      `window.__nodeWebview && window.__nodeWebview._handleSend(${JSON.stringify(event)}, ${payload})`
    )
  }

  // ===== WebView 操作 =====

  close(): Promise<void> {
    return this.send('close')
  }
  requestRedraw(): Promise<void> {
    return this.send('request_redraw')
  }
  setUrl(url: string): Promise<void> {
    return this.send('set_url', url)
  }
  loadUrlWithHeaders(url: string, headers: Record<string, string>): Promise<void> {
    return this.send('load_url_with_headers', { url, headers })
  }
  url(): Promise<string> {
    return this.send('url')
  }
  evaluateScript(script: string): Promise<void> {
    return this.send('evaluate_script', script)
  }
  evaluateScriptReturnResult(script: string): Promise<string> {
    return this.send('evaluate_script_with_callback', script)
  }
  print(): Promise<void> {
    return this.send('print')
  }
  openDevtools(): Promise<void> {
    return this.send('open_devtools')
  }
  closeDevtools(): Promise<void> {
    return this.send('close_devtools')
  }
  isDevtoolsOpen(): Promise<boolean> {
    return this.send('is_devtools_open')
  }
  zoom(scale: number): Promise<void> {
    return this.send('zoom', scale)
  }
  scaleFactor(): Promise<number> {
    return this.send('scale_factor')
  }
  clearAllBrowsingData(): Promise<void> {
    return this.send('clear_all_browsing_data')
  }
  setBackgroundColor(color: [number, number, number, number]): Promise<void> {
    return this.send('set_background_color', color)
  }
  setWindowBackgroundColor(color: [number, number, number, number] | null): Promise<void> {
    return this.send('set_window_background_color', color)
  }

  // ===== 窗口位置/尺寸 =====

  position(): Promise<Position> {
    return this.send('inner_position')
  }
  outerPosition(): Promise<Position> {
    return this.send('outer_position')
  }
  setPosition(x: number, y: number): Promise<void> {
    return this.send('set_outer_position', { x, y })
  }
  size(): Promise<Size> {
    return this.send('inner_size')
  }
  setSize(width: number, height: number): Promise<Size> {
    return this.send('set_inner_size', { width, height })
  }
  outerSize(): Promise<Size> {
    return this.send('outer_size')
  }
  setMinSize(width: number, height: number): Promise<void> {
    return this.send('set_min_inner_size', { width, height })
  }
  setMaxSize(width: number, height: number): Promise<void> {
    return this.send('set_max_inner_size', { width, height })
  }
  setInnerSizeConstraints(constraints: WindowSizeConstraints): Promise<void> {
    return this.send('set_inner_size_constraints', constraints)
  }

  // ===== 窗口属性 =====

  setTitle(title: string): Promise<void> {
    return this.send('set_title', title)
  }
  title(): Promise<string> {
    return this.send('title')
  }
  setVisible(visible: boolean): Promise<void> {
    return this.send('set_visible', visible)
  }
  isVisible(): Promise<boolean> {
    return this.send('is_visible')
  }
  setResizable(resizable: boolean): Promise<void> {
    return this.send('set_resizable', resizable)
  }
  isResizable(): Promise<boolean> {
    return this.send('is_resizable')
  }
  setMinimizable(minimizable: boolean): Promise<void> {
    return this.send('set_minimizable', minimizable)
  }
  isMinimizable(): Promise<boolean> {
    return this.send('is_minimizable')
  }
  setMaximizable(maximizable: boolean): Promise<void> {
    return this.send('set_maximizable', maximizable)
  }
  isMaximizable(): Promise<boolean> {
    return this.send('is_maximizable')
  }
  setClosable(closable: boolean): Promise<void> {
    return this.send('set_closable', closable)
  }
  isClosable(): Promise<boolean> {
    return this.send('is_closable')
  }
  setEnabledButtons(buttons: WindowButton[] = ['close', 'maximize', 'minimize']): Promise<void> {
    return this.send('set_enabled_buttons', buttons)
  }
  enabledButtons(): Promise<WindowButton[]> {
    return this.send('enabled_buttons')
  }
  minimized(): Promise<void> {
    return this.send('set_minimized', true)
  }
  unminimized(): Promise<void> {
    return this.send('set_minimized', false)
  }
  isMinimized(): Promise<boolean> {
    return this.send('is_minimized')
  }
  maximized(): Promise<void> {
    return this.send('set_maximized', true)
  }
  unmaximized(): Promise<void> {
    return this.send('set_maximized', false)
  }
  isMaximized(): Promise<boolean> {
    return this.send('is_maximized')
  }

  // ===== 全屏/装饰/层级 =====

  fullscreen(monitorId?: null | Monitor['monitorId']): Promise<void> {
    return this.send('fullscreen', monitorId ?? null)
  }
  unfullscreen(): Promise<void> {
    return this.send('unfullscreen')
  }
  isFullscreen(): Promise<Monitor['monitorId'] | boolean> {
    return this.send('is_fullscreen')
  }
  setDecorations(decorations: boolean): Promise<void> {
    return this.send('set_decorations', decorations)
  }
  isDecorated(): Promise<boolean> {
    return this.send('is_decorated')
  }
  /**
   * 设置无边框窗口
   *
   * 无边框窗口可通过 CSS `-webkit-app-region: drag` 拖动窗口，
   * `-webkit-app-region: no-drag` 排除交互区域。
   */
  borderless(borderless = true): Promise<void> {
    return this.setDecorations(!borderless)
  }
  async isBorderless(): Promise<boolean> {
    return !(await this.isDecorated())
  }
  setAlwaysOnTop(top = true): Promise<void> {
    return this.send('set_always_on_top', top)
  }
  isAlwaysOnTop(): Promise<boolean> {
    return this.send('is_always_on_top')
  }
  setAlwaysOnBottom(bottom = true): Promise<void> {
    return this.send('set_always_on_bottom', bottom)
  }

  // ===== 外观/行为 =====

  setIcon(icon: string): Promise<void> {
    return this.send('set_window_icon', icon)
  }
  focus(): Promise<void> {
    return this.send('focus_window')
  }
  hasFocus(): Promise<boolean> {
    return this.send('has_focus')
  }
  setImePosition(position: Position): Promise<void> {
    return this.send('set_ime_position', position)
  }
  setProgressBar(progress: ProgressBarOptions | null): Promise<void> {
    return this.send('set_progress_bar', progress)
  }
  requestUserAttention(type: UserAttentionType = 'critical'): Promise<void> {
    return this.send('request_user_attention', type)
  }
  cancelUserAttentionRequest(): Promise<void> {
    return this.send('request_user_attention', null)
  }
  setTheme(theme: Theme | 'default' | null): Promise<void> {
    return this.send('set_theme', theme === 'default' ? null : theme)
  }
  theme(): Promise<Theme | null> {
    return this.send('theme')
  }
  setContentProtection(enabled: boolean): Promise<void> {
    return this.send('set_content_protection', enabled)
  }
  setVisibleOnAllWorkspaces(visible: boolean): Promise<void> {
    return this.send('set_visible_on_all_workspaces', visible)
  }

  // ===== 光标 =====

  setCursorIcon(cursor: CursorIcon): Promise<void> {
    return this.send('set_cursor_icon', cursor)
  }
  setCursorPosition(position: Position): Promise<void> {
    return this.send('set_cursor_position', position)
  }
  setCursorGrab(grab: boolean): Promise<void> {
    return this.send('set_cursor_grab', grab)
  }
  setCursorVisible(visible: boolean): Promise<void> {
    return this.send('set_cursor_visible', visible)
  }
  dragWindow(): Promise<void> {
    return this.send('drag_window')
  }
  dragResizeWindow(direction: ResizeDirection): Promise<void> {
    return this.send('drag_resize_window', direction)
  }
  setIgnoreCursorEvents(ignore: boolean): Promise<void> {
    return this.send('set_ignore_cursor_events', ignore)
  }
  cursorPosition(): Promise<Position> {
    return this.send('cursor_position')
  }

  // ===== 内部方法 =====

  private async create(props: BrowserWindowAttributes): Promise<WindowId> {
    const id = await this.send('create', props)
    this._id = id
    this.app._emit(this.label, 'created', id)
    return id
  }

  private send<T = any>(method: string, data?: any): Promise<T> {
    return this.app._sendIoMessage({ method, data, label: this.label })
  }

  private get app() {
    const app = getCurrentApplication()
    if (!app) throw new Error('Application has not been created')
    return app
  }
}
