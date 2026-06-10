import { uid } from './utils'
import type { AppEvent, Monitor, ReceiveMessage, SendMessage, RPCInterface, MenuOptions, ProtocolHandler, ApplicationOptions } from './types'
import type BrowserWindow from './window'
import { Menu } from './menu'

// 加载 native 模块
interface NativeModule {
  start: (callback: (json: string) => void) => void
  sendCommand: (id: string, method: string, label: string, data: string) => void
  evaluateScript: (id: string, label: string, script: string) => void
  rpcInvoke: (id: string, label: string, method: string, data: string) => void
  rpcResolve: (id: string, label: string, rpcId: number, data: string, error?: string | null) => void
  rpcSend: (id: string, label: string, event: string, data: string) => void
  protocolResponse: (id: string, label: string, requestId: string, statusCode: number, headers: string, body: string) => void
}

function loadNative(): NativeModule {
  const { platform, arch } = process
  const candidates = [
    `./index.${platform}-${arch}.node`,
    `./taowry.${platform}-${arch}.node`,
  ]
  for (const name of candidates) {
    try { return require(name) } catch { /* continue */ }
  }
  // 尝试通过 require.resolve 查找
  const path = require('path')
  const fs = require('fs')
  const searchDirs = [__dirname, path.join(__dirname, '..'), path.join(__dirname, '..', '..'), process.cwd()]
  for (const dir of searchDirs) {
    for (const name of candidates) {
      const p = path.join(dir, name)
      if (fs.existsSync(p)) return require(p)
    }
  }
  throw new Error(`[taowry] 找不到 native 模块 (.node 文件)，请运行 napi build --platform --release 编译`)
}

const native = loadNative()

/** 用于存储全局唯一的 Application 实例 */
const CURRENT_APP_KEY = '__taowryApp'

type Listener = (data?: any) => void
type PendingCallback = {
  resolve: (value: any) => void
  reject: (reason?: any) => void
}
type QueuedMessage = PendingCallback & {
  msg: SendMessage
}

/** 获取当前 Application 实例 */
export const getCurrentApplication = (): Application | undefined => {
  return (globalThis as any)[CURRENT_APP_KEY]
}

/** 将字符串或 Uint8Array 编码为 base64 */
function encodeToBase64(data: string | Uint8Array): string {
  return typeof data === 'string'
    ? Buffer.from(data, 'utf-8').toString('base64')
    : Buffer.from(data).toString('base64')
}


/**
 * Application - 应用实例管理器
 * 通过 napi 原生模块与 Rust 端通信，管理应用生命周期
 */
export default class Application {
  private callbacks: Record<string, PendingCallback> = {}
  private listeners: Record<string, Record<string, Listener[]>> = {}
  private windows: Record<string, BrowserWindow> = {}
  /** 就绪前排队的消息 */
  private queue: QueuedMessage[] = []
  /** Rust 事件循环是否就绪 */
  private ready = false
  /** views:// 自定义协议 handler（应用级，所有窗口共享） */
  private _protocol?: ProtocolHandler

  constructor(options: ApplicationOptions = {}) {
    const current = getCurrentApplication()
    if (current && current.ready) {
      throw new Error('Application already exists')
    }
    ;(globalThis as any)[CURRENT_APP_KEY] = this
    if (options.protocol) {
      this._protocol = options.protocol
    }
  }

  /** 启动 Rust 事件循环 */
  async run(): Promise<void> {
    return new Promise<void>(resolve => {
      native.start((json: string) => {
        try {
          const msg: ReceiveMessage = JSON.parse(json)
          this.handleIoMessage(msg)
          // ready 事件触发时 resolve
          if (msg.type === 'appEvent' && msg.method === 'ready') {
            resolve()
          }
        } catch (e) {
          console.error('[taowry] Failed to parse message:', json, e)
        }
      })
    })
  }

  /** 退出应用 */
  quit(): Promise<void> {
    if (!this.ready) return Promise.resolve()
    return this._sendIoMessage({ label: 'app', method: 'app_quit' })
  }

  /** 获取 WebView 引擎版本号 */
  webviewVersion(): Promise<string> {
    return this._sendIoMessage({ label: 'app', method: 'webview_version' })
  }

  /** 获取所有窗口标签列表 */
  windowLabels(): Promise<string[]> {
    return this._sendIoMessage({ label: 'app', method: 'app_window_labels' })
  }

  /** 设置应用全局菜单 (仅 macOS 有效) */
  async setApplicationMenu(menu: MenuOptions): Promise<void> {
    const menuLabel = 'app:auto-menu'
    const autoMenu = new Menu(menuLabel, menu)
    await autoMenu.created
    return this._sendIoMessage({ label: 'app', method: 'set_application_menu', data: menuLabel })
  }

  /** 设置 Dock 菜单 (仅 macOS 有效) */
  async setDockMenu(menu: MenuOptions): Promise<void> {
    const menuLabel = 'app:dock-menu'
    const autoMenu = new Menu(menuLabel, menu)
    await autoMenu.created
    return this._sendIoMessage({ label: 'app', method: 'set_dock_menu', data: menuLabel })
  }

  /** 显示 Dock 图标 (仅 macOS 有效) */
  showDockIcon(): Promise<void> {
    return this._sendIoMessage({ label: 'app', method: 'show_dock_icon' })
  }

  /** 隐藏 Dock 图标 (仅 macOS 有效) */
  hideDockIcon(): Promise<void> {
    return this._sendIoMessage({ label: 'app', method: 'hide_dock_icon' })
  }

  /** 设置 Dock 图标 badge 文本，空字符串清除 (仅 macOS 有效) */
  setDockBadge(text: string): Promise<void> {
    return this._sendIoMessage({ label: 'app', method: 'set_dock_badge', data: text })
  }

  /** 让 Dock 图标弹跳 (仅 macOS 有效) */
  bounceDock(): Promise<void> {
    return this._sendIoMessage({ label: 'app', method: 'bounce_dock' })
  }

  // ===== 显示器 =====

  /** 获取所有显示器列表 */
  monitors(): Promise<Monitor[]> {
    return this._sendIoMessage({ label: 'app', method: 'get_monitor_list' })
  }

  /** 获取主显示器 */
  primaryMonitor(): Promise<Monitor | null> {
    return this._sendIoMessage({ label: 'app', method: 'primary_monitor' })
  }

  /** 获取指定坐标处的显示器 */
  monitorFromPoint(x: number, y: number): Promise<Monitor | null> {
    return this._sendIoMessage({ label: 'app', method: 'monitor_from_point', data: { x, y } })
  }

  /** 通过标签名获取已创建的窗口实例 */
  getWindow<T extends RPCInterface = {}>(label: string): BrowserWindow<T> | undefined {
    return this.windows[label] as BrowserWindow<T> | undefined
  }

  /** @internal 注册窗口实例 */
  _registerWindow<T extends RPCInterface = any>(window: BrowserWindow<T>) {
    if (this.windows[window.label]) {
      throw new Error(`BrowserWindow '${window.label}' already exists`)
    }
    this.windows[window.label] = window as any
  }

  /** @internal 注销窗口实例 */
  _unregisterWindow(label: string) {
    delete this.windows[label]
  }

  /** @internal 发送命令到 Rust 端，返回 Promise */
  _sendIoMessage(msg: SendMessage): Promise<any> {
    return new Promise((resolve, reject) => {
      msg.id = uid()
      if (!this.ready) {
        this.queue.push({ msg, resolve, reject })
        return
      }
      this.writeMessage(msg, resolve, reject)
    })
  }

  /** @internal 执行 WebView JS 并异步返回结果 */
  _evaluateScript(label: string, script: string): Promise<string> {
    return new Promise((resolve, reject) => {
      const id = uid()
      this.callbacks[id] = { resolve, reject }
      try {
        native.evaluateScript(id, label, script)
      } catch (error) {
        delete this.callbacks[id]
        reject(error)
      }
    })
  }

  /** @internal Host→WebView RPC 请求（延迟响应） */
  _rpcInvoke(label: string, method: string, data: any): Promise<any> {
    return new Promise((resolve, reject) => {
      const id = uid()
      this.callbacks[id] = { resolve, reject }
      try {
        native.rpcInvoke(id, label, method, JSON.stringify(data ?? null))
      } catch (error) {
        delete this.callbacks[id]
        reject(error)
      }
    })
  }

  /** @internal Host 回复 WebView→Host RPC 请求 */
  _rpcResolve(label: string, rpcId: number, data: any, error?: string): void {
    native.rpcResolve(uid(), label, rpcId, JSON.stringify(data ?? null), error ?? null)
  }

  /** @internal Host→WebView 单向 RPC 消息 */
  _rpcSend(label: string, event: string, data: any): void {
    native.rpcSend(uid(), label, event, JSON.stringify(data ?? null))
  }

  on<T extends keyof AppEvent>(event: T, callback: (data: AppEvent[T]) => void): () => void
  on(label: string, event: string, callback: Listener): () => void
  on(labelOrEvent: string, eventOrCallback: string | Listener, callback?: Listener): () => void {
    if (typeof eventOrCallback === 'function') {
      return this.addListener('app', labelOrEvent, eventOrCallback)
    }
    return this.addListener(labelOrEvent, eventOrCallback, callback as Listener)
  }

  once<T extends keyof AppEvent>(event: T, callback: (data: AppEvent[T]) => void): () => void
  once(label: string, event: string, callback: Listener): () => void
  once(labelOrEvent: string, eventOrCallback: string | Listener, callback?: Listener): () => void {
    if (typeof eventOrCallback === 'function') {
      return this.addOnceListener('app', labelOrEvent, eventOrCallback)
    }
    return this.addOnceListener(labelOrEvent, eventOrCallback, callback as Listener)
  }

  /** @internal 移除事件监听 */
  _off(label: string, event: string, callback: Listener) {
    const listeners = this.listeners[label]?.[event]
    if (!listeners) return
    this.listeners[label][event] = listeners.filter(item => item !== callback)
  }

  /** @internal 触发事件 */
  _emit(label: string, event: string, data: any) {
    const listeners = this.listeners[label]?.[event] ?? []
    listeners.slice().forEach(callback => callback(data))
  }

  private addListener(label: string, event: string, callback: Listener) {
    if (!this.listeners[label]) this.listeners[label] = {}
    if (!this.listeners[label][event]) this.listeners[label][event] = []
    this.listeners[label][event].push(callback)
    return () => this._off(label, event, callback)
  }

  private addOnceListener(label: string, event: string, callback: Listener) {
    const wrapper: Listener = data => {
      callback(data)
      this._off(label, event, wrapper)
    }
    return this.addListener(label, event, wrapper)
  }

  private writeMessage(
    msg: SendMessage,
    resolve: PendingCallback['resolve'],
    reject: PendingCallback['reject']
  ) {
    this.callbacks[msg.id as string] = { resolve, reject }
    try {
      native.sendCommand(
        msg.id as string,
        msg.method,
        msg.label,
        JSON.stringify(msg.data ?? null)
      )
    } catch (error) {
      delete this.callbacks[msg.id as string]
      reject(error)
    }
  }

  private flushQueue() {
    const queue = this.queue.splice(0)
    queue.forEach((item: any) => {
      if (item._protocolSend) {
        // 排队的协议响应，直接发送
        item._protocolSend()
      } else {
        this.writeMessage(item.msg, item.resolve, item.reject)
      }
    })
  }

  private handleIoMessage(msg: ReceiveMessage) {
    switch (msg.type) {
      case 'response': {
        const callback = msg.id ? this.callbacks[msg.id] : undefined
        if (!callback) return
        delete this.callbacks[msg.id as string]
        if (msg.error) callback.reject(new Error(msg.error))
        else callback.resolve(msg.data)
        break
      }
      case 'appEvent':
        if (msg.method === 'ready') {
          this.ready = true
          this.flushQueue()
        }
        this._emit('app', msg.method, msg.data)
        break
      case 'windowEvent':
        if (msg.method === 'destroy') this._unregisterWindow(msg.label)
        if (msg.method === 'protocolRequest') {
          this._handleProtocolRequest(msg.label, msg.data)
          break
        }
        this._emit(msg.label, msg.method, msg.data)
        break
      case 'trayEvent':
        this._emit(`tray:${msg.label}`, msg.method, msg.data)
        break
      case 'menuEvent':
        this._emit(`menu:${msg.label}`, msg.method, msg.data)
        break
    }
  }

  // ===== views:// 自定义协议 =====

  /** 注册或替换应用级 views:// 协议 handler（所有窗口共享） */
  setProtocol(handler: ProtocolHandler): void {
    this._protocol = handler
  }

  /** 移除协议 handler */
  removeProtocol(): void {
    this._protocol = undefined
  }

  /** 处理 views:// 协议请求（内部方法，直接使用 native.protocolResponse） */
  private async _handleProtocolRequest(
    label: string,
    data: { requestId: string; uri: string; method: string; headers: Record<string, string>; body?: string }
  ) {
    const respond = (statusCode: number, headers: Record<string, string>, body: string) => {
      const id = uid()
      const send = () => native.protocolResponse(id, label, data.requestId, statusCode, JSON.stringify(headers), body)
      if (!this.ready) {
        // 未就绪时排队，ready 后发送
        this.queue.push({
          msg: { id, method: '__protocolResponse__', label, data: null },
          resolve: () => {},
          reject: () => {},
        })
        // 将实际发送存储在排队消息中
        const last = this.queue[this.queue.length - 1] as any
        last._protocolSend = send
      } else {
        send()
      }
    }

    if (!this._protocol) {
      respond(404, { 'content-type': 'text/plain' }, encodeToBase64('No protocol handler registered'))
      return
    }
    try {
      const requestHeaders = new Headers()
      if (data.headers) {
        for (const [k, v] of Object.entries(data.headers)) requestHeaders.append(k, v)
      }
      const requestInit: RequestInit = { method: data.method, headers: requestHeaders }
      if (data.body && data.method !== 'GET' && data.method !== 'HEAD') {
        requestInit.body = new Uint8Array(Buffer.from(data.body, 'base64'))
      }
      const request = new Request(data.uri, requestInit)
      const response = await this._protocol(request)

      const body = new Uint8Array(await response.arrayBuffer())
      const responseHeaders: Record<string, string> = {}
      response.headers.forEach((value, key) => { responseHeaders[key] = value })

      respond(response.status, responseHeaders, encodeToBase64(body))
    } catch (err: any) {
      respond(500, { 'content-type': 'text/plain' }, encodeToBase64(err?.message || String(err)))
    }
  }

  private rejectAll(error: Error) {
    const callbacks = this.callbacks
    this.callbacks = {}
    Object.keys(callbacks).forEach(id => callbacks[id].reject(error))
    const queue = this.queue.splice(0)
    queue.forEach(item => item.reject(error))
  }
}
