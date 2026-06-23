import { readFileSync } from 'fs'
import { join, extname } from 'path'
import type {
  AppEvent,
  Monitor,
  ReceiveMessage,
  RPCInterface,
  MenuOptions,
  ProtocolHandler,
  ApplicationOptions
} from './types'
import type BrowserWindow from './window'
import { Menu } from './menu.js'
import { native, json, parse, initNative } from './native-module.js'
import { EventBus, type Listener } from './event-bus.js'

/** 用于存储全局唯一的 Application 实例 */
const CURRENT_APP_KEY = '__taowryApp'


/** 获取当前 Application 实例 */
export const getCurrentApplication = (): Application | undefined => {
  return (globalThis as any)[CURRENT_APP_KEY]
}

/**
 * Application - 应用实例管理器
 * 构造函数即启动 Rust 事件循环，无需调用 run()
 */
export default class Application {
  private bus = new EventBus()
  private windows: Record<string, BrowserWindow> = {}
  private _protocol?: ProtocolHandler
  private _assetsDir?: string
  private _appName?: string
  private _ready = false

  constructor(options: ApplicationOptions = {}) {
    const current = getCurrentApplication()
    if (current && current._ready) {
      throw new Error('Application already exists')
    }
    ;(globalThis as any)[CURRENT_APP_KEY] = this
    if (options.protocol) {
      this._protocol = options.protocol
    }
    if (options.assets) {
      this._assetsDir = options.assets
    }
    if (options.appName) {
      this._appName = options.appName
    }

    // 初始化 native 模块
    initNative()

    // 传递应用名称到 Rust（用于应用范围目录）
    if (this._appName) {
      native.setAppName(this._appName)
    }

    // 立即启动 Rust 事件循环
    native.start((raw: string) => {
      try {
        const msg = parse<ReceiveMessage>(raw)
        this.handleEvent(msg)
      } catch (e) {
        console.error('[taowry] Failed to parse message:', raw, e)
      }
    })
  }

  /** 是否就绪 */
  get ready(): boolean {
    return this._ready
  }

  /** 等待就绪 */
  whenReady(): Promise<void> {
    if (this._ready) return Promise.resolve()
    return new Promise(resolve => {
      this.once('ready' as any, () => resolve())
    })
  }

  /** 退出应用 */
  quit(): void {
    native.quit()
  }

  /** 获取 WebView 引擎版本号 */
  webviewVersion(): string {
    return native.webviewVersion()
  }

  /** 获取所有窗口标签列表 */
  windowLabels(): string[] {
    return parse<string[]>(native.windowLabels())
  }

  /** 设置应用全局菜单 (仅 macOS 有效) */
  async setApplicationMenu(menu: MenuOptions): Promise<void> {
    const menuLabel = 'app:auto-menu'
    const autoMenu = new Menu(menuLabel, menu)
    await autoMenu.created
    native.setApplicationMenu(menuLabel)
  }

  /** 设置 Dock 菜单 (仅 macOS 有效) */
  async setDockMenu(menu: MenuOptions): Promise<void> {
    const menuLabel = 'app:dock-menu'
    const autoMenu = new Menu(menuLabel, menu)
    await autoMenu.created
    native.setDockMenu(menuLabel)
  }

  /** 显示 Dock 图标 (仅 macOS 有效) */
  showDockIcon(): void {
    native.showDockIcon()
  }

  /** 隐藏 Dock 图标 (仅 macOS 有效) */
  hideDockIcon(): void {
    native.hideDockIcon()
  }

  /** 设置 Dock 图标 badge 文本，空字符串清除 (仅 macOS 有效) */
  setDockBadge(text: string): void {
    native.setDockBadge(text)
  }

  /** 让 Dock 图标弹跳 (仅 macOS 且窗口不在前台时有效) */
  bounceDock(): void {
    native.bounceDock()
  }

  // ===== 显示器 =====

  /** 获取所有显示器列表 */
  monitors(): Monitor[] {
    return parse<Monitor[]>(native.getMonitorList())
  }

  /** 获取主显示器 */
  primaryMonitor(): Monitor | null {
    return parse(native.primaryMonitor())
  }

  /** 获取指定坐标处的显示器 */
  monitorFromPoint(x: number, y: number): Monitor | null {
    return parse(native.monitorFromPoint(json({ x, y })))
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

  /** @internal 注销窗口实例，同时清理关联的事件监听器 */
  _unregisterWindow(label: string) {
    delete this.windows[label]
    this.bus.removeNamespace(label)
  }

  /** @internal 清理指定标签的所有事件监听器（用于托盘等非窗口实体） */
  _cleanupListeners(label: string) {
    this.bus.removeNamespace(label)
  }

  // ===== RPC =====

  /** @internal 执行 WebView JS 并异步返回结果 */
  _evaluateScript(label: string, script: string): Promise<string> {
    return new Promise(resolve => {
      native.evaluateScript(label, script, (result: string) => {
        resolve(result)
      })
    })
  }

  /** @internal Host→WebView RPC 请求（延迟响应） */
  _rpcInvoke(label: string, method: string, data: any): Promise<any> {
    return new Promise((resolve, reject) => {
      native.rpcInvoke(label, method, json(data ?? null), (result: string) => {
        try {
          const parsed = parse(result)
          if (parsed.error) reject(new Error(parsed.error))
          else resolve(parsed.data)
        } catch {
          resolve(result)
        }
      })
    })
  }

  /** @internal Host 回复 WebView→Host RPC 请求 */
  _rpcResolve(label: string, rpcId: number, data: any, error?: string): void {
    native.rpcResolve(label, rpcId, json(data ?? null), error ?? null)
  }

  /** @internal Host→WebView 单向 RPC 消息 */
  _rpcSend(label: string, event: string, data: any): void {
    native.rpcSend(label, event, json(data ?? null))
  }

  // ===== 事件系统 =====

  on<T extends keyof AppEvent>(event: T, callback: (data: AppEvent[T]) => void): () => void
  on(label: string, event: string, callback: Listener): () => void
  on(labelOrEvent: string, eventOrCallback: string | Listener, callback?: Listener): () => void {
    if (typeof eventOrCallback === 'function') {
      return this.bus.on('app', labelOrEvent, eventOrCallback)
    }
    return this.bus.on(labelOrEvent, eventOrCallback, callback as Listener)
  }

  once<T extends keyof AppEvent>(event: T, callback: (data: AppEvent[T]) => void): () => void
  once(label: string, event: string, callback: Listener): () => void
  once(labelOrEvent: string, eventOrCallback: string | Listener, callback?: Listener): () => void {
    if (typeof eventOrCallback === 'function') {
      return this.bus.once('app', labelOrEvent, eventOrCallback)
    }
    return this.bus.once(labelOrEvent, eventOrCallback, callback as Listener)
  }

  /** @internal 移除事件监听 */
  _off(label: string, event: string, callback: Listener) {
    this.bus.off(label, event, callback)
  }

  /** @internal 触发事件 */
  _emit(label: string, event: string, data: any) {
    this.bus.emit(label, event, data)
  }

  // ===== 事件处理 =====

  private handleEvent(msg: ReceiveMessage) {
    switch (msg.type) {
      case 'appEvent':
        if (msg.method === 'ready') {
          this._ready = true
        }
        this._emit('app', msg.method, msg.data)
        break
      case 'windowEvent':
        if (msg.method === 'destroy') this._unregisterWindow(msg.label)
        if (msg.method === 'protocolRequest') {
          this._handleProtocolRequest(msg.label, msg.data)
          break
        }
        if (msg.method === 'assetsRequest') {
          this._handleAssetsRequest(msg.label, msg.data)
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

  /** 协议请求公共响应助手 */
  private _respond(
    label: string,
    requestId: string,
    statusCode: number,
    headers: Record<string, string>,
    body: Buffer
  ) {
    native.protocolResponse(label, requestId, statusCode, json(headers), body)
  }

  /** 将 IPC 原始数据构建为标准 Request */
  private _buildRequest(data: {
    uri: string
    method: string
    headers: Record<string, string>
    body?: string
  }): Request {
    const requestHeaders = new Headers()
    if (data.headers) {
      for (const [k, v] of Object.entries(data.headers)) requestHeaders.append(k, v)
    }
    const requestInit: RequestInit = { method: data.method, headers: requestHeaders }
    if (data.body && data.method !== 'GET' && data.method !== 'HEAD') {
      requestInit.body = new Uint8Array(Buffer.from(data.body, 'base64'))
    }
    return new Request(data.uri, requestInit)
  }

  /** 处理 views:// 协议请求 */
  private async _handleProtocolRequest(
    label: string,
    data: { requestId: string; uri: string; method: string; headers: Record<string, string>; body?: string }
  ) {
    if (!this._protocol) {
      this._respond(
        label,
        data.requestId,
        404,
        { 'content-type': 'text/plain' },
        Buffer.from('No protocol handler registered')
      )
      return
    }
    try {
      const request = this._buildRequest(data)
      const response = await this._protocol(request)
      const body = Buffer.from(new Uint8Array(await response.arrayBuffer()))
      const responseHeaders: Record<string, string> = {}
      response.headers.forEach((value, key) => {
        responseHeaders[key] = value
      })
      this._respond(label, data.requestId, response.status, responseHeaders, body)
    } catch (err: any) {
      this._respond(
        label,
        data.requestId,
        500,
        { 'content-type': 'text/plain' },
        Buffer.from(err?.message || String(err))
      )
    }
  }

  // ===== assets:// 静态资源协议 =====

  private static readonly MIME_MAP: Record<string, string> = {
    '.html': 'text/html; charset=utf-8',
    '.htm': 'text/html; charset=utf-8',
    '.js': 'application/javascript; charset=utf-8',
    '.mjs': 'application/javascript; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
    '.gif': 'image/gif',
    '.svg': 'image/svg+xml',
    '.webp': 'image/webp',
    '.ico': 'image/x-icon',
    '.woff': 'font/woff',
    '.woff2': 'font/woff2',
    '.ttf': 'font/ttf',
    '.otf': 'font/otf',
    '.wasm': 'application/wasm'
  }

  /** 处理 assets:// 静态资源请求 */
  private _handleAssetsRequest(label: string, data: { requestId: string; uri: string }) {
    if (!this._assetsDir) {
      this._respond(
        label,
        data.requestId,
        404,
        { 'content-type': 'text/plain' },
        Buffer.from('No assets directory configured')
      )
      return
    }

    // TS 端创建窗口时已注入内部 host __taowry__：
    //   assets://index.html → assets://__taowry__/index.html
    // Rust 原样转发 URI，TS 端提取 path 部分作为文件路径
    //   assets://__taowry__/index.html → index.html
    //   assets://__taowry__/page/style.css → page/style.css
    const clean = data.uri.replace(/^assets:\/\/[^/]*\//, '')

    if (!clean || clean.includes('..')) {
      this._respond(label, data.requestId, 403, { 'content-type': 'text/plain' }, Buffer.from('Forbidden'))
      return
    }

    const filePath = join(this._assetsDir, clean)

    try {
      const content = readFileSync(filePath) // 返回 Buffer，直传零拷贝
      const ext = extname(filePath).toLowerCase()
      const mime = Application.MIME_MAP[ext] || 'application/octet-stream'
      this._respond(
        label,
        data.requestId,
        200,
        { 'content-type': mime, 'access-control-allow-origin': '*' },
        content
      )
    } catch {
      this._respond(
        label,
        data.requestId,
        404,
        { 'content-type': 'text/plain' },
        Buffer.from(`File not found: ${clean}`)
      )
    }
  }
}
