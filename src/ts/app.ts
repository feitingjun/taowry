import { ChildProcessWithoutNullStreams, spawn } from 'child_process'
import { getBinaryPath, uid } from './utils'
import { AppEvent, Monitor, ReceiveMessage, SendMessage, RPCInterface, MenuOptions } from './types'
import type BrowserWindow from './window'
import { Menu } from './menu'

/** IPC 消息前缀，用于在 stdin/stdout 中区分 JSON 消息和普通输出 */
const IO_CHANNEL_PREFIX = '_ioc:'
/** 用于存储全局唯一的 Application 实例 */
const CURRENT_APP_KEY = '__nodeWebviewApp'

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

/**
 * Application - 应用实例管理器
 * 负责启动 Rust 子进程、管理 IPC 通信和应用生命周期
 */
export default class Application {
  private callbacks: Record<string, PendingCallback> = {}
  private listeners: Record<string, Record<string, Listener[]>> = {}
  private windows: Record<string, BrowserWindow> = {}
  private childProcess?: ChildProcessWithoutNullStreams
  /** 就绪前排队的消息 */
  private queue: QueuedMessage[] = []
  /** stdout 读取缓冲区 */
  private stdoutBuffer = ''
  private runPromise?: Promise<number | null>
  /** Rust 子进程是否就绪 */
  private ready = false

  constructor() {
    const current = getCurrentApplication()
    if (current && current.childProcess && !current.childProcess.killed) {
      throw new Error('Application already exists')
    }
    ;(globalThis as any)[CURRENT_APP_KEY] = this
  }

  /** 启动 Rust 子进程，返回退出码 */
  async run(): Promise<number | null> {
    if (this.runPromise) return this.runPromise

    const binaryPath = getBinaryPath()
    this.childProcess = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe']
    })

    this.childProcess.stdout.on('data', data => this.handleStdout(data))
    this.childProcess.stderr.on('data', data => {
      const message = data.toString()
      if (message.trim()) console.error(message)
    })
    this.childProcess.on('error', error => {
      this.rejectAll(error)
    })

    this.runPromise = new Promise(resolve => {
      this.childProcess?.on('exit', code => {
        this.ready = false
        this.rejectAll(new Error(`Rust application exited with code ${code}`))
        resolve(code)
      })
    })

    return this.runPromise
  }

  /** 退出应用 */
  quit(): Promise<void> {
    if (!this.childProcess || this.childProcess.killed) {
      this.ready = false
      return Promise.resolve()
    }
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

  /** @internal 发送 IPC 消息到 Rust 子进程，返回 Promise */
  _sendIoMessage(msg: SendMessage): Promise<any> {
    return new Promise((resolve, reject) => {
      msg.id = uid()
      if (!this.childProcess || this.childProcess.killed || !this.ready) {
        this.queue.push({ msg, resolve, reject })
        return
      }
      this.writeMessage(msg, resolve, reject)
    })
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
    if (!this.childProcess || this.childProcess.killed) {
      reject(new Error('Application is not running'))
      return
    }
    this.callbacks[msg.id as string] = { resolve, reject }
    this.childProcess.stdin.write(`${IO_CHANNEL_PREFIX}${JSON.stringify(msg)}\n`, error => {
      if (!error) return
      delete this.callbacks[msg.id as string]
      reject(error)
    })
  }

  private flushQueue() {
    const queue = this.queue.splice(0)
    queue.forEach(({ msg, resolve, reject }) => this.writeMessage(msg, resolve, reject))
  }

  private handleStdout(data: Buffer) {
    this.stdoutBuffer += data.toString()
    let newlineIndex = this.stdoutBuffer.indexOf('\n')
    while (newlineIndex >= 0) {
      const line = this.stdoutBuffer.slice(0, newlineIndex)
      this.stdoutBuffer = this.stdoutBuffer.slice(newlineIndex + 1)
      this.handleLine(line)
      newlineIndex = this.stdoutBuffer.indexOf('\n')
    }
  }

  private handleLine(line: string) {
    if (!line.startsWith(IO_CHANNEL_PREFIX)) {
      if (line.trim()) console.log(line)
      return
    }

    let msg: ReceiveMessage
    try {
      msg = JSON.parse(line.slice(IO_CHANNEL_PREFIX.length))
    } catch (error) {
      console.error(`响应消息格式错误：${line}`)
      return
    }
    this.handleIoMessage(msg)
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

  private rejectAll(error: Error) {
    const callbacks = this.callbacks
    this.callbacks = {}
    Object.keys(callbacks).forEach(id => callbacks[id].reject(error))
    const queue = this.queue.splice(0)
    queue.forEach(item => item.reject(error))
  }
}
