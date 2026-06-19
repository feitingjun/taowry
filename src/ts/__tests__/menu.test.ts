/**
 * menu.ts normalizeMenuItem 单元测试
 */
import { describe, test, expect } from 'bun:test'
import { normalizeMenuItem } from '../menu.js'

describe('normalizeMenuItem', () => {
  test('自动生成 id', () => {
    const result = normalizeMenuItem({ text: 'Item' }, 'menu:0')
    expect(result.id).toBe('menu:0')
  })

  test('保留已有 id', () => {
    const result = normalizeMenuItem({ id: 'custom', text: 'Item' }, 'fallback')
    expect(result.id).toBe('custom')
  })

  test('有 items 则 type 默认 submenu', () => {
    const result = normalizeMenuItem(
      { text: 'Parent', items: [{ text: 'Child' }] },
      'm:0'
    )
    expect(result.type).toBe('submenu')
    expect(result.items![0].id).toBe('m:0:0')
  })

  test('有 checked 且无 type 则设为 check', () => {
    const result = normalizeMenuItem({ text: 'Toggle', checked: true }, 'm:0')
    expect(result.type).toBe('check')
  })

  test('已指定 type 则保留', () => {
    const result = normalizeMenuItem(
      { text: 'Parent', type: 'normal' as any, items: [{ text: 'Child' }] },
      'm:0'
    )
    expect(result.type).toBe('normal')
  })

  test('嵌套子菜单 id 层级正确', () => {
    const result = normalizeMenuItem(
      {
        text: 'A',
        items: [
          { text: 'A1' },
          { text: 'A2', items: [{ text: 'A2a' }] }
        ]
      },
      'm:0'
    )
    expect(result.items![0].id).toBe('m:0:0')
    expect(result.items![1].id).toBe('m:0:1')
    expect(result.items![1].items![0].id).toBe('m:0:1:0')
  })
})
