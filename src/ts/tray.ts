import { Rect, TrayIconEvent, TrayIconOptions, MenuOptions } from './types'
import { getCurrentApplication } from './app'
import { Menu } from './menu'

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

    if (options.menu) {
      // 自动创建内部菜单
      const menuLabel = `${label}:auto-menu`
      this._autoMenu = new Menu(menuLabel, options.menu)
      this.created = this._autoMenu.created.then(() => {
        return app()._sendIoMessage({
          label,
          method: 'create_tray',
          data: { ...options, menu: menuLabel },
        })
      })
    } else {
      const { menu, ...restOptions } = options
      this.created = app()._sendIoMessage({
        label,
        method: 'create_tray',
        data: restOptions,
      })
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
  setIcon(icon: string | null): Promise<void> {
    return app()._sendIoMessage({ label: this.label, method: 'set_tray_icon', data: icon })
  }

  /** 设置托盘菜单 */
  async setMenu(menu: MenuOptions | null): Promise<void> {
    if (!menu) {
      return app()._sendIoMessage({ label: this.label, method: 'set_tray_menu', data: null })
    }
    const menuLabel = `${this.label}:auto-menu-${++this._menuCounter}`
    const autoMenu = new Menu(menuLabel, menu)
    this._autoMenu = autoMenu
    await autoMenu.created
    return app()._sendIoMessage({ label: this.label, method: 'set_tray_menu', data: menuLabel })
  }

  /** 设置鼠标悬停提示 */
  setTooltip(tooltip: string | null): Promise<void> {
    return app()._sendIoMessage({ label: this.label, method: 'set_tray_tooltip', data: tooltip })
  }

  /** 设置托盘标题 (仅 macOS) */
  setTitle(title: string | null): Promise<void> {
    return app()._sendIoMessage({ label: this.label, method: 'set_tray_title', data: title })
  }

  /** 设置托盘可见性 */
  setVisible(visible: boolean): Promise<void> {
    return app()._sendIoMessage({ label: this.label, method: 'set_tray_visible', data: visible })
  }

  /** 获取托盘图标区域信息 */
  rect(): Promise<Rect | null> {
    return app()._sendIoMessage({ label: this.label, method: 'tray_rect' })
  }

  /** 移除托盘图标 */
  remove(): Promise<void> {
    return app()._sendIoMessage({ label: this.label, method: 'remove_tray' })
  }
}

function app() {
  const current = getCurrentApplication()
  if (!current) throw new Error('Create an Application before creating tray icons')
  return current
}
