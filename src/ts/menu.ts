import type { MenuItemOptions } from './types'
import { getCurrentApplication } from './app'

/**
 * Menu - 内部菜单栏管理类
 * 由 Tray/Application/BrowserWindow 自动创建，不对外导出
 */
export class Menu {
  readonly label: string
  private _items: MenuItemOptions[]
  created: Promise<void>

  constructor(label: string, items: MenuItemOptions[] = []) {
    this.label = label
    this._items = items.map((item, index) => normalizeMenuItem(item, `${label}:${index}`))
    this.created = app()._sendIoMessage({
      label,
      method: 'create_menu',
      data: this._items
    })
  }

  /** 获取菜单项列表 */
  get items(): MenuItemOptions[] {
    return this._items
  }
}

/** 标准化菜单项，确保每个菜单项都有 id 和正确的 type */
function normalizeMenuItem(item: MenuItemOptions, fallbackId: string): MenuItemOptions {
  const id = item.id ?? fallbackId
  const normalized: MenuItemOptions = { ...item, id }
  if (item.items) {
    normalized.type = item.type ?? 'submenu'
    normalized.items = item.items.map((child, index) => normalizeMenuItem(child, `${id}:${index}`))
  }
  if (item.checked !== undefined && !normalized.type) normalized.type = 'check'
  return normalized
}

function app() {
  const current = getCurrentApplication()
  if (!current) throw new Error('Create an Application before creating menus')
  return current
}
