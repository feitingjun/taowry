/**
 * EventBus 进阶测试 — 边界条件和压力场景
 */
import { describe, test, expect, mock } from 'bun:test'
import { EventBus } from '../event-bus.js'

describe('EventBus advanced', () => {
  test('同一事件大量监听器', () => {
    const bus = new EventBus()
    const cbs = Array.from({ length: 100 }, () => mock())
    cbs.forEach(cb => bus.on('ns', 'evt', cb))
    bus.emit('ns', 'evt', 42)
    cbs.forEach(cb => expect(cb).toHaveBeenCalledTimes(1))
  })

  test('on 返回的取消函数可多次调用不报错', () => {
    const bus = new EventBus()
    const cb = mock()
    const unsub = bus.on('ns', 'evt', cb)
    unsub()
    expect(() => unsub()).not.toThrow()
    expect(() => unsub()).not.toThrow()
  })

  test('同一个回调注册多次，off 只移除一次', () => {
    const bus = new EventBus()
    const cb = mock()
    bus.on('ns', 'evt', cb)
    bus.on('ns', 'evt', cb) // 注册两次
    bus.off('ns', 'evt', cb) // 移除一次（filter 会移除所有匹配）
    bus.emit('ns', 'evt', null)
    expect(cb).toHaveBeenCalledTimes(0)
  })

  test('once 返回的取消函数可提前取消', () => {
    const bus = new EventBus()
    const cb = mock()
    const unsub = bus.once('ns', 'evt', cb)
    unsub()
    bus.emit('ns', 'evt', null)
    expect(cb).toHaveBeenCalledTimes(0)
  })

  test('removeNamespace 后 on 不再有效', () => {
    const bus = new EventBus()
    const cb = mock()
    bus.on('ns', 'evt', cb)
    bus.removeNamespace('ns')
    // 重新在同一个 ns 注册
    const cb2 = mock()
    bus.on('ns', 'evt', cb2)
    bus.emit('ns', 'evt', null)
    expect(cb).toHaveBeenCalledTimes(0)
    expect(cb2).toHaveBeenCalledTimes(1)
  })

  test('emit 无数据时 listener 收到 undefined', () => {
    const bus = new EventBus()
    let received: any = 'NOT_CALLED'
    bus.on('ns', 'evt', (data) => { received = data })
    bus.emit('ns', 'evt', undefined)
    expect(received).toBeUndefined()
  })

  test('多个命名空间互不干扰', () => {
    const bus = new EventBus()
    const results: string[] = []
    bus.on('a', 'x', () => results.push('a'))
    bus.on('b', 'x', () => results.push('b'))
    bus.on('a', 'y', () => results.push('ay'))
    bus.emit('a', 'x', null)
    bus.emit('b', 'x', null)
    bus.emit('a', 'y', null)
    expect(results).toEqual(['a', 'b', 'ay'])
  })
})
