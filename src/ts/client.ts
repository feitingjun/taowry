/**
 * taowry/client - WebView 端客户端 SDK
 * 通过 taowry/client 子路径导入
 * 用于与 taowry Host 端进行双向 RPC 通信，以及直接控制当前窗口
 */

import type { RPCInterface, RPCPromise, RPCSchema } from './rpc-types.js'
import type { DefineRPCConfig } from './rpc-types.js'

export type { RPCInterface, RPCPromise, RPCSchema }

// ===== 窗口控制 API 类型 =====

/** 拖拽调整方向 */
type ResizeDirection =
  | 'east'
  | 'north'
  | 'northEast'
  | 'northWest'
  | 'south'
  | 'southEast'
  | 'southWest'
  | 'west'

/** 尺寸 */
type Size = { width: number; height: number }

/** 坐标位置 */
type Position = { x: number; y: number }

/**
 * 当前窗口控制接口
 *
 * 直接操作当前 WebView 所在的窗口，无需 RPC 中转。
 * 由 Rust 端直接执行，延迟 ≤ 16ms。
 */
export interface TaowryWindow {
  // ── Fire-and-forget（同步调用，无返回值）──

  /** 关闭窗口 */
  close(): void
  /** 最小化窗口 */
  minimize(): void
  /** 取消最小化 */
  unminimize(): void
  /** 最大化窗口 */
  maximize(): void
  /** 取消最大化 */
  unmaximize(): void
  /** 聚焦窗口 */
  focus(): void
  /** 设置窗口可见性 */
  setVisible(visible: boolean): void
  /** 设置窗口标题 */
  setTitle(title: string): void
  /** 设置窗口尺寸 */
  setSize(width: number, height: number): void
  /** 设置窗口位置 */
  setPosition(x: number, y: number): void
  /** 设置窗口是否可调整大小 */
  setResizable(resizable: boolean): void
  /** 设置窗口是否置顶 */
  setAlwaysOnTop(top: boolean): void
  /** 设置窗口装饰（标题栏、边框） */
  setDecorations(decorations: boolean): void
  /** 进入全屏 */
  fullscreen(): void
  /** 退出全屏 */
  unfullscreen(): void
  /** 打开开发者工具 */
  openDevtools(): void
  /** 关闭开发者工具 */
  closeDevtools(): void
  /** 拖拽窗口（需鼠标左键按下） */
  dragWindow(): void
  /** 拖拽调整窗口大小（需鼠标左键按下，macOS 不支持） */
  dragResizeWindow(direction: ResizeDirection): void
  /** 设置 WebView URL */
  setUrl(url: string): void
  /** 打印页面 */
  print(): void

  // ── Request-Response（返回 Promise）──

  /** 获取窗口是否最小化 */
  isMinimized(): Promise<boolean>
  /** 获取窗口是否最大化 */
  isMaximized(): Promise<boolean>
  /** 获取窗口是否全屏 */
  isFullscreen(): Promise<boolean>
  /** 获取窗口是否可见 */
  isVisible(): Promise<boolean>
  /** 获取窗口是否可调整大小 */
  isResizable(): Promise<boolean>
  /** 获取窗口是否置顶 */
  isAlwaysOnTop(): Promise<boolean>
  /** 获取窗口是否有装饰 */
  isDecorated(): Promise<boolean>
  /** 获取窗口是否有焦点 */
  hasFocus(): Promise<boolean>
  /** 获取开发者工具是否打开 */
  isDevtoolsOpen(): Promise<boolean>
  /** 获取窗口客户区域尺寸 */
  size(): Promise<Size>
  /** 获取整个窗口物理尺寸 */
  outerSize(): Promise<Size>
  /** 获取窗口客户区域位置 */
  position(): Promise<Position>
  /** 获取窗口位置（含边框/标题栏） */
  outerPosition(): Promise<Position>
  /** 获取窗口标题 */
  title(): Promise<string>
  /** 获取当前 WebView URL */
  url(): Promise<string>
  /** 获取窗口缩放因子 */
  scaleFactor(): Promise<number>
}

/** Webview 端 RPC 实例（由 defineRPC 返回） */
export interface WebviewRPCInstance<T extends RPCInterface> {
  /** 调用 host 端的 request 方法（request-response） */
  requests: RPCPromise<T['host'], 'requests'>
  /** 向 host 端发送消息（fire-and-forget） */
  messages: RPCPromise<T['host'], 'messages'>
  /** 监听 host 端发来的消息（本端定义的 messages） */
  on(event: keyof RPCPromise<T['webview'], 'messages'>, callback: (...args: any[]) => void): () => void
  /** 移除监听 */
  off(event: keyof RPCPromise<T['webview'], 'messages'>, callback: (...args: any[]) => void): void
}

// ===== 工具 API 类型 =====
/** 保存文件对话框选项 */
interface SaveFileOptions {
  filters?: FilterItem[]
  directory?: string
  fileName?: string
}

/** 消息对话框级别 */
type MessageLevel = 'info' | 'warning' | 'error'

/** 消息对话框按钮配置 */
type MessageButtons = 'ok' | 'okCancel' | 'yesNo' | 'yesNoCancel' | string[]

/** 消息对话框选项 */
interface ShowMessageOptions {
  title: string
  body?: string
  level?: MessageLevel
  buttons?: MessageButtons
}
// ===== 全局声明（window.__taowry） =====

declare global {
  interface Window {
    __taowry?: {
      defineRPC(config: {
        requests?: Record<string, (data: any) => any>
        messages?: Record<string, ((data: any) => void) | undefined>
      }): {
        requests: Record<string, (data: any) => Promise<any>>
        messages: Record<string, (data: any) => void>
        on(event: string, callback: (data: any) => void): () => void
        off(event: string, callback: (data: any) => void): void
      }
      /** 原生窗口控制接口（由桥接脚本注入，Rust 端直接执行） */
      window: TaowryWindow
      /** 原生工具接口（由桥接脚本注入，Rust 端直接执行） */
    }
    ipc?: {
      postMessage(message: string): void
    }
  }
}

// ===== 导出 API =====

/**
 * 获取当前窗口控制实例
 *
 * 直接操作当前 WebView 所在的窗口，无需 RPC 中转。
 * 由 Rust 端通过命令队列直接执行，延迟 ≤ 16ms（一帧）。
 *
 * @example
 * ```typescript
 * import { getCurrentWindow } from 'taowry/client'
 *
 * const win = getCurrentWindow()
 *
 * // 关闭窗口
 * win.close()
 *
 * // 设置标题
 * win.setTitle('新标题')
 *
 * // 设置窗口尺寸
 * win.setSize(1024, 768)
 *
 * // 最小化/最大化
 * win.minimize()
 * win.maximize()
 *
 * // 无边框窗口的自定义拖拽区域
 * document.querySelector('.titlebar')?.addEventListener('mousedown', () => {
 *   win.dragWindow()
 * })
 *
 * // 异步获取窗口状态
 * const maximized = await win.isMaximized()
 * const { width, height } = await win.size()
 * ```
 */
export function getCurrentWindow(): TaowryWindow {
  if (!window.__taowry?.window) {
    throw new Error('window.__taowry.window is not available. Make sure you are running inside a taowry WebView.')
  }
  return window.__taowry.window
}
/**
 * 创建类型安全的 WebView 端 RPC 实例
 *
 * @example
 * ```typescript
 * import { defineRPC, RPCInterface } from 'taowry/client'
 *
 * interface MyRPC extends RPCInterface {
 *   host: {
 *     requests: {
 *       echo: (data: { msg: string }) => { received: string }
 *     }
 *     messages: {
 *       update: { message: string }
 *     }
 *   }
 *   webview: {
 *     requests: {
 *       renderData: (data: { items: string[] }) => { count: number }
 *     }
 *     messages: {
 *       ready: { url: string }
 *     }
 *   }
 * }
 *
 * const rpc = defineRPC<MyRPC>({
 *   requests: {
 *     renderData: (data) => ({ count: data.items.length })
 *   },
 *   messages: {
 *     ready: (data) => console.log('就绪:', data.url)
 *   }
 * })
 *
 * // 调用 Host 端方法
 * const result = await rpc.requests.echo({ msg: 'hello' })
 *
 * // 监听 Host 端发送的消息
 * rpc.on('update', (data) => console.log(data.message))
 *
 * // 向 Host 端发送消息（fire-and-forget，同步调用）
 * rpc.messages.ready({ url: location.href })
 * ```
 */
export function defineRPC<T extends RPCInterface>(
  config: DefineRPCConfig<T['webview']>
): WebviewRPCInstance<T> {
  if (!window.__taowry) {
    throw new Error('window.__taowry is not available. Make sure you are running inside a taowry WebView.')
  }
  const rawRpc = window.__taowry.defineRPC({
    requests: ((config as any)?.requests ?? {}) as Record<string, (data: any) => any>,
    messages: (config?.messages ?? {}) as Record<string, ((data: any) => void) | undefined>
  })

  return rawRpc as unknown as WebviewRPCInstance<T>
}

