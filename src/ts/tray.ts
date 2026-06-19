import { readFileSync } from 'fs'
import type { Rect, TrayIconEvent, TrayIconOptions, MenuOptions } from './types'
import { getCurrentApplication } from './app.js'
import { Menu } from './menu.js'
import { native, json, parseOrNull } from './native-module.js'

/**
 * Tray - 系统托盘图标
 * 用于在系统托盘区域显示图标和菜单
 */
export class Tray {
  readonly label: string
  created: Promise<void>
  private _autoMenu?: Menu
  private _menuCounter = 0

  constructor(label: string, options: TrayIconOptions = {}) {
    this.label = label
    // 读取托盘图标，转化为base64传入以支持虚拟路径
    if (options.icon) {
      options.icon = readFileSync(options.icon).toBase64()
    }
    if (options.menu) {
      const menuLabel = `${label}:auto-menu`
      this._autoMenu = new Menu(menuLabel, options.menu)
      // Menu creation is synchronous now, so create tray immediately
      native.createTray(label, json({ ...options, menu: menuLabel }))
      this.created = Promise.resolve()
    } else {
      const { menu, ...restOptions } = options
      native.createTray(label, json(restOptions))
      this.created = Promise.resolve()
    }
  }

  /** 监听托盘事件 */
  on<T extends keyof TrayIconEvent>(event: T, callback: (data: TrayIconEvent[T]) => void) {
    return app().on(`tray:${this.label}`, event, callback as any)
  }

  once<T extends keyof TrayIconEvent>(event: T, callback: (data: TrayIconEvent[T]) => void) {
    return app().once(`tray:${this.label}`, event, callback as any)
  }

  /** 设置托盘图标 */
  setIcon(icon: string | null): void {
    native.setTrayIcon(this.label, icon ? readFileSync(icon) : Buffer.alloc(0))
  }

  /** 设置托盘菜单 */
  async setMenu(menu: MenuOptions | null): Promise<void> {
    if (!menu) {
      native.setTrayMenu(this.label, json(null))
      return
    }
    const menuLabel = `${this.label}:auto-menu-${++this._menuCounter}`
    const autoMenu = new Menu(menuLabel, menu)
    this._autoMenu = autoMenu
    await autoMenu.created
    native.setTrayMenu(this.label, json(menuLabel))
  }

  /** 设置鼠标悬停提示 */
  setTooltip(tooltip: string | null): void {
    native.setTrayTooltip(this.label, json(tooltip))
  }

  /** 设置托盘标题 (仅 macOS) */
  setTitle(title: string | null): void {
    native.setTrayTitle(this.label, json(title))
  }

  /** 设置托盘可见性 */
  setVisible(visible: boolean): void {
    native.setTrayVisible(this.label, json(visible))
  }

  /** 获取托盘图标区域信息 */
  rect(): Rect | null {
    return parseOrNull(native.trayRect(this.label))
  }

  /** 移除托盘图标，同时清理事件监听器 */
  remove(): void {
    native.removeTray(this.label)
    const current = getCurrentApplication()
    if (current) {
      current._cleanupListeners(`tray:${this.label}`)
    }
  }
}

function app() {
  const current = getCurrentApplication()
  if (!current) throw new Error('Create an Application before creating tray icons')
  return current
}
