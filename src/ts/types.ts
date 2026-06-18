/** 窗口唯一标识 */
export type WindowId = string

/** 尺寸 */
export type Size = { width: number; height: number }

/** 坐标位置 */
export type Position = { x: number; y: number }

/** 矩形区域 */
export type Rect = Position & Size

/** 显示器信息 */
export type Monitor = Size &
  Position & {
    monitorId: number
    name?: string | null
    scaleFactor: number
  }

/** 窗口控制按钮 */
export type WindowButton = 'close' | 'minimize' | 'maximize'
/** 标题栏样式 */
export type TitleBarStyle = 'visible' | 'hidden' | 'hiddenInset'
/** 主题 */
export type Theme = 'light' | 'dark'
/** 用户注意力请求类型 */
export type UserAttentionType = 'critical' | 'informational'
/** 拖拽调整方向 */
export type ResizeDirection =
  | 'east'
  | 'north'
  | 'northEast'
  | 'northWest'
  | 'south'
  | 'southEast'
  | 'southWest'
  | 'west'
/** 进度条状态 */
export type ProgressState = 'none' | 'normal' | 'indeterminate' | 'paused' | 'error'

/** 光标图标类型 */
export type CursorIcon =
  | 'default'
  | 'crosshair'
  | 'hand'
  | 'arrow'
  | 'move'
  | 'text'
  | 'wait'
  | 'help'
  | 'progress'
  | 'notAllowed'
  | 'contextMenu'
  | 'cell'
  | 'verticalText'
  | 'alias'
  | 'copy'
  | 'noDrop'
  | 'grab'
  | 'grabbing'
  | 'allScroll'
  | 'zoomIn'
  | 'zoomOut'
  | 'eResize'
  | 'nResize'
  | 'neResize'
  | 'nwResize'
  | 'sResize'
  | 'seResize'
  | 'swResize'
  | 'wResize'
  | 'ewResize'
  | 'nsResize'
  | 'neswResize'
  | 'nwseResize'
  | 'colResize'
  | 'rowResize'

/** 进度条配置 */
export interface ProgressBarOptions {
  state?: ProgressState
  progress?: number
  desktopFilename?: string
}

/** views:// 协议 handler 函数签名（标准 Web Request/Response） */
export type ProtocolHandler = (request: Request) => Response | Promise<Response>

/** Application 构造选项 */
export interface ApplicationOptions {
  /** views:// 动态协议 handler（应用级，所有窗口共享） */
  protocol?: ProtocolHandler
  /** assets:// 静态资源目录，Rust 直接从文件系统加载 */
  assets?: string
}

/** 窗口尺寸约束 */
export interface WindowSizeConstraints {
  minWidth?: number
  minHeight?: number
  maxWidth?: number
  maxHeight?: number
}

/** 创建窗口时的配置属性 */
export interface BrowserWindowAttributes<T extends RPCInterface = {}> {
  /** WebView 加载的 URL */
  url?: string
  /** WebView 加载的 HTML */
  html?: string
  /** 请求头 */
  headers?: Record<string, string>
  /** WebView 背景色 [r, g, b, a] */
  backgroundColor?: [number, number, number, number]
  /** 窗口背景色 [r, g, b, a] */
  windowBackgroundColor?: [number, number, number, number]
  /** 是否启用开发者工具 */
  devtools?: boolean
  /** 自定义 User-Agent */
  userAgent?: string
  /** 是否启用剪贴板 */
  clipboard?: boolean
  /** 是否在首次鼠标点击时激活窗口 (macOS) */
  acceptFirstMouse?: boolean
  /** 页面加载前执行的脚本 */
  initializationScripts?: string[]
  /** 是否允许导航 */
  navigationAllowed?: boolean
  /** 是否允许打开新窗口 */
  newWindowAllowed?: boolean
  /** 是否阻止拖拽默认行为 */
  dragDropPreventDefault?: boolean
  /** 是否允许下载 */
  downloadAllowed?: boolean
  /** 下载目录 */
  downloadPath?: string
  /** 窗口菜单配置 */
  menu?: MenuOptions
  /** 窗口 RPC 配置（支持多窗口复用） */
  rpc?: T['host']

  /** 窗口初始宽度 */
  width?: number
  /** 窗口初始高度 */
  height?: number
  /** 最小窗口宽度 */
  minWidth?: number
  /** 最小窗口高度 */
  minHeight?: number
  /** 最大窗口宽度 */
  maxWidth?: number
  /** 最大窗口高度 */
  maxHeight?: number
  /** 窗口初始 X 位置 */
  x?: number
  /** 窗口初始 Y 位置 */
  y?: number
  /** 是否可调整大小 */
  resizable?: boolean
  /** 是否可最小化 */
  minimizable?: boolean
  /** 是否可最大化 */
  maximizable?: boolean
  /** 是否可关闭 */
  closable?: boolean
  /** 启用的控制按钮 */
  enabledButtons?: WindowButton[]
  /** 窗口标题 */
  title?: string
  /** 是否最大化 */
  maximized?: boolean
  /** 是否可见 */
  visible?: boolean
  /** 是否透明 */
  transparent?: boolean
  /**
   * 是否无边框
   *
   * 无边框窗口可通过 CSS `-webkit-app-region: drag` 拖动窗口，
   * `-webkit-app-region: no-drag` 排除交互区域。
   */
  borderless?: boolean
  /** 是否显示装饰 */
  decorations?: boolean
  /** 标题栏样式 (macOS) */
  titleBarStyle?: TitleBarStyle
  /** 交通灯按钮位置偏移 (macOS, titleBarStyle='hiddenInset' 时有效) */
  trafficLightPosition?: Position
  /** 窗口图标路径 */
  windowIcon?: string
  /** 窗口主题 */
  theme?: Theme
  /** 是否启用内容保护 */
  contentProtected?: boolean
  /** 是否在所有工作区可见 */
  visibleOnAllWorkspaces?: boolean
  /** 是否激活窗口 */
  active?: boolean
  /** 是否获取焦点 */
  focused?: boolean
  /** 是否全屏 */
  fullscreen?: boolean | Monitor['monitorId']
  /** 是否置顶 */
  alwaysOnTop?: boolean
  /** 是否置底 */
  alwaysOnBottom?: boolean
}

/** 窗口事件映射 */
export interface WindowEvent {
  created: WindowId
  close: void
  destroy: void
  move: Position
  resize: Size
  focus: void
  blur: void
  cursorMove: Position
  cursorEnter: void
  cursorOut: void
  theme: Theme
  droppedFile: { path: string }
  hoveredFile: { path: string }
  hoveredFileCancelled: void
  receivedImeText: string
  keyboardInput: any
  modifiersChanged: { shift: boolean; control: boolean; alt: boolean; super: boolean }
  mouseWheel: any
  mouseInput: any
  touchpadPressure: { pressure: number; stage: number }
  axisMotion: { axis: string; value: number }
  touch: any
  scaleFactorChanged: { scaleFactor: number; innerSize: Size }
  decorationsClick: void
  ipcMessage: { url: string; body: string }
  navigation: { url: string }
  newWindow: { url: string }
  documentTitleChanged: { title: string }
  pageLoad: { event: 'started' | 'finished'; url: string }
  dragDrop: any
  downloadStarted: { url: string; path: string }
  downloadCompleted: { url: string; path?: string | null; success: boolean }
}

/** 预定义菜单项类型 */
export type PredefinedMenuItem =
  | 'separator'
  | 'copy'
  | 'cut'
  | 'paste'
  | 'selectAll'
  | 'undo'
  | 'redo'
  | 'minimize'
  | 'maximize'
  | 'fullscreen'
  | 'hide'
  | 'hideOthers'
  | 'showAll'
  | 'closeWindow'
  | 'quit'
  | 'services'
  | 'bringAllToFront'

/** 菜单项配置 */
export interface MenuItemOptions {
  id?: string
  type?: 'normal' | 'check' | 'submenu' | 'separator' | 'predefined'
  text?: string
  enabled?: boolean
  checked?: boolean
  accelerator?: string
  item?: PredefinedMenuItem
  items?: MenuItemOptions[]
}

/** 菜单配置 (即菜单项数组) */
export type MenuOptions = MenuItemOptions[]

/** 托盘图标配置 */
export interface TrayIconOptions {
  icon?: string
  tooltip?: string
  title?: string
  menu?: MenuOptions
  tempDirPath?: string
  iconAsTemplate?: boolean
  menuOnLeftClick?: boolean
}

/** 托盘事件映射 */
export interface TrayIconEvent {
  click: any
  doubleClick: any
  enter: any
  move: any
  leave: any
}

/** 应用事件映射 */
export interface AppEvent {
  ready: void
  quit: void
}

/** 从 Rust 子进程接收的 IPC 消息格式 */
export interface ReceiveMessage {
  type: 'windowEvent' | 'appEvent' | 'trayEvent' | 'menuEvent'
  method: string
  label: string
  data?: any
}

/**RPC定义辅助类型 */
export type RPCSchema<
  T extends {
    requests?: Record<string, (...args: any[]) => any>
    messages?: Record<string, any>
  }
> = {
  [K in keyof T]: K extends 'messages'
    ? {
        [K2 in keyof T[K]]: (data: T[K][K2]) => void
      }
    : {
        [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => infer R
          ? (...args: A) => Promise<R> | R
          : never
      }
}

/** RPC 接口定义 — 分别定义 host 端和 webview 端的方法 */
export interface RPCInterface {
  host?: {
    /** 请求方法（request-response，webview 调用 host） */
    requests?: Record<string, (...args: any[]) => any>
    /** 消息（fire-and-forget，双向） */
    messages?: Record<string, (...args: any[]) => void>
  }
  webview?: {
    /** 请求方法（request-response，host 调用 webview） */
    requests?: Record<string, (...args: any[]) => any>
    /** 消息（fire-and-forget，双向） */
    messages?: Record<string, (...args: any[]) => void>
  }
}

/** Window.rpc 方法类型转换 */
export type RPCPromise<T, K extends PropertyKey> = T extends object
  ? K extends keyof T
    ? T[K] extends object
      ? K extends 'messages'
        ? {
            [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => any ? (...args: A) => void : never
          }
        : {
            [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => infer R
              ? (...args: A) => Promise<Awaited<R>>
              : never
          }
      : {}
    : {}
  : {}

/**defineRPC参数类型 */
export type DefineRPCConfig<T> = Omit<T, 'messages'> & {
  messages?: 'messages' extends keyof T
    ? T['messages'] extends object
      ? { [K in keyof T['messages']]?: T['messages'][K] }
      : T['messages']
    : never
}

/** Host 端 RPC 实例（由 BrowserWindow 创建，通过 win.rpc 访问） */
export interface HostRPCInstance<T extends RPCInterface> {
  /** 调用 webview 端的 request 方法（request-response） */
  requests: RPCPromise<T['webview'], 'requests'>
  /** 向 webview 端发送消息（fire-and-forget） */
  messages: RPCPromise<T['webview'], 'messages'>
  /** 监听 webview 端发来的消息（本端定义的 messages） */
  on(event: keyof RPCPromise<T['host'], 'messages'>, callback: (...args: any[]) => void): () => void
  /** 移除监听 */
  off(event: keyof RPCPromise<T['host'], 'messages'>, callback: (...args: any[]) => void): void
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
