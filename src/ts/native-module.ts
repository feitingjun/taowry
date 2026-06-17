/**
 * Native 模块接口定义 + 加载
 *
 * 每个 napi 函数直接返回实际值（字符串、布尔、void），
 * TypeScript 端直接调用并接收返回值。
 */
import { existsSync } from 'fs'
import { join } from 'path'

export interface NativeModule {
  // ===== 生命周期 =====
  start: (callback: (json: string) => void) => void
  quit: () => void

  // ===== 应用级 =====
  windowLabels: () => string
  webviewVersion: () => string
  createWindow: (label: string, data: string) => string

  // ===== 窗口 WebView 操作 =====
  windowClose: (label: string) => void
  windowRequestRedraw: (label: string) => void
  windowSetUrl: (label: string, url: string) => void
  windowLoadUrlWithHeaders: (label: string, data: string) => void
  windowUrl: (label: string) => string
  windowEvaluateScript: (label: string, script: string) => void
  windowPrint: (label: string) => void
  windowOpenDevtools: (label: string) => void
  windowCloseDevtools: (label: string) => void
  windowIsDevtoolsOpen: (label: string) => boolean
  windowZoom: (label: string, scale: number) => void
  windowClearAllBrowsingData: (label: string) => void
  windowSetBackgroundColor: (label: string, data: string) => void
  windowSetWindowBackgroundColor: (label: string, data: string) => void

  // ===== 窗口尺寸/位置 =====
  windowInnerPosition: (label: string) => string
  windowOuterPosition: (label: string) => string
  windowSetOuterPosition: (label: string, data: string) => void
  windowInnerSize: (label: string) => string
  windowSetInnerSize: (label: string, data: string) => string
  windowOuterSize: (label: string) => string
  windowSetMinInnerSize: (label: string, data: string) => void
  windowSetMaxInnerSize: (label: string, data: string) => void
  windowSetInnerSizeConstraints: (label: string, data: string) => void
  windowScaleFactor: (label: string) => number
  windowRequestUserAttention: (label: string, data: string) => void

  // ===== 窗口属性 =====
  windowSetTitle: (label: string, title: string) => void
  windowTitle: (label: string) => string
  windowSetVisible: (label: string, visible: boolean) => void
  windowIsVisible: (label: string) => boolean
  windowFocus: (label: string) => void
  windowHasFocus: (label: string) => boolean
  windowSetResizable: (label: string, value: boolean) => void
  windowIsResizable: (label: string) => boolean
  windowSetMinimizable: (label: string, value: boolean) => void
  windowIsMinimizable: (label: string) => boolean
  windowSetMaximizable: (label: string, value: boolean) => void
  windowIsMaximizable: (label: string) => boolean
  windowSetClosable: (label: string, value: boolean) => void
  windowIsClosable: (label: string) => boolean
  windowSetEnabledButtons: (label: string, data: string) => void
  windowEnabledButtons: (label: string) => string
  windowSetMinimized: (label: string, value: boolean) => void
  windowIsMinimized: (label: string) => boolean
  windowSetMaximized: (label: string, value: boolean) => void
  windowIsMaximized: (label: string) => boolean
  windowFullscreen: (label: string, data: string) => void
  windowUnfullscreen: (label: string) => void
  windowIsFullscreen: (label: string) => string
  windowSetDecorations: (label: string, value: boolean) => void
  windowIsDecorated: (label: string) => boolean
  windowSetAlwaysOnTop: (label: string, value: boolean) => void
  windowIsAlwaysOnTop: (label: string) => boolean
  windowSetAlwaysOnBottom: (label: string, value: boolean) => void
  windowSetWindowIcon: (label: string, icon: Buffer) => void
  windowSetImePosition: (label: string, data: string) => void
  windowSetProgressBar: (label: string, data: string) => void
  windowSetTheme: (label: string, data: string) => void
  windowTheme: (label: string) => string
  windowSetContentProtection: (label: string, value: boolean) => void
  windowSetVisibleOnAllWorkspaces: (label: string, value: boolean) => void
  windowId: (label: string) => string

  // ===== 光标 =====
  windowSetCursorIcon: (label: string, cursor: string) => void
  windowSetCursorPosition: (label: string, data: string) => void
  windowSetCursorGrab: (label: string, value: boolean) => void
  windowSetCursorVisible: (label: string, value: boolean) => void
  windowDragWindow: (label: string) => void
  windowDragResizeWindow: (label: string, direction: string) => void
  windowSetIgnoreCursorEvents: (label: string, value: boolean) => void
  windowCursorPosition: (label: string) => string

  // ===== 特殊操作（需要回调）=====
  evaluateScript: (label: string, script: string, callback: (result: string) => void) => void
  rpcInvoke: (label: string, method: string, data: string, callback: (result: string) => void) => void
  rpcResolve: (label: string, rpcId: number, data: string, error: string | null) => void
  rpcSend: (label: string, event: string, data: string) => void
  protocolResponse: (
    label: string,
    requestId: string,
    statusCode: number,
    headers: string,
    body: Buffer
  ) => void

  // ===== 菜单 =====
  createMenu: (label: string, data: string) => void
  appendMenuItem: (menuLabel: string, data: string) => string
  setApplicationMenu: (menuLabel: string) => void
  setWindowMenu: (label: string, menuLabel: string) => void
  setMenuItemEnabled: (itemId: string, enabled: boolean) => void
  setMenuItemText: (itemId: string, text: string) => void
  setMenuItemChecked: (itemId: string, checked: boolean) => void
  isMenuItemChecked: (itemId: string) => boolean

  // ===== 托盘 =====
  createTray: (label: string, data: string) => void
  removeTray: (label: string) => void
  setTrayIcon: (label: string, icon: Buffer) => void
  setTrayMenu: (label: string, data: string) => void
  setTrayTooltip: (label: string, data: string) => void
  setTrayTitle: (label: string, data: string) => void
  setTrayVisible: (label: string, data: string) => void
  trayRect: (label: string) => string

  // ===== Dock =====
  showDockIcon: () => void
  hideDockIcon: () => void
  setDockBadge: (text: string) => void
  bounceDock: () => void
  setDockMenu: (menuLabel: string) => void

  // ===== 显示器 =====
  primaryMonitor: () => string
  getMonitorList: () => string
  monitorFromPoint: (data: string) => string
}

// ===== Native 模块懒加载 =====

let _native: NativeModule | undefined

/**
 * 初始化 native 模块
 *
 * 由 Application 构造函数自动调用，通常无需手动调用。
 *
 * @param binary - 直接传入已加载的原生模块，不传则自动查找
 */
export function initNative(binary?: any): void {
  if (_native) return

  // 用户直接传入已加载的模块
  if (binary) {
    _native = binary as NativeModule
    return
  }

  // 自动查找：项目根目录 → npm 包安装目录
  const filename = 'taowry.node'
  const searchPaths: string[] = []

  // 1. 项目根目录（process.cwd()）
  searchPaths.push(join(process.cwd(), filename))

  // 2. npm 包安装目录（__dirname 向上查找到 taowry 包根）
  let dir = __dirname
  searchPaths.push(join(dir, '..', filename))

  for (const p of searchPaths) {
    if (existsSync(p)) {
      _native = require(p)
      return
    }
  }

  throw new Error(
    `[taowry] 找不到 native 模块: ${filename}\n` +
      `已搜索:\n  ${searchPaths.join('\n  ')}\n` +
      `请运行 npm install（自动下载）或 napi build --platform（本地编译）\n` +
      `或在 new Application({ binary: require('taowry.node') }) 中显式传入`
  )
}

/** native 模块代理，首次访问时自动初始化 */
export const native: NativeModule = new Proxy({} as NativeModule, {
  get(_, prop: string) {
    if (!_native) initNative()
    return (_native as any)[prop]
  }
})

// ===== JSON 序列化辅助函数 =====

/** 将任意值序列化为 JSON 字符串（null/undefined → "null"） */
export function json(data: any): string {
  return JSON.stringify(data ?? null)
}

/** 反序列化 JSON 字符串，"null" 返回 null */
export function parseOrNull<T = any>(str: string): T | null {
  return str === 'null' ? null : (JSON.parse(str) as T)
}

/** 反序列化 JSON 字符串（保证非 null） */
export function parse<T = any>(str: string): T {
  return JSON.parse(str) as T
}
