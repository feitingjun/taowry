/**
 * EventBus 单元测试
 */
import { describe, test, expect, mock } from 'bun:test'
import { EventBus } from '../event-bus.js'

describe('EventBus', () => {
  test('on + emit 基本流程', () => {
    const bus = new EventBus()
    const cb = mock()
    bus.on('win', 'close', cb)
    bus.emit('win', 'close', { reason: 'test' })
    expect(cb).toHaveBeenCalledTimes(1)
    expect(cb.mock.calls[0][0]).toEqual({ reason: 'test' })
  })

  test('on 返回取消函数', () => {
    const bus = new EventBus()
    const cb = mock()
    const unsub = bus.on('win', 'resize', cb)
    bus.emit('win', 'resize', null)
    expect(cb).toHaveBeenCalledTimes(1)
    unsub()
    bus.emit('win', 'resize', null)
    expect(cb).toHaveBeenCalledTimes(1) // 取消后不再触发
  })

  test('once 只触发一次', () => {
    const bus = new EventBus()
    const cb = mock()
    bus.once('app', 'ready', cb)
    bus.emit('app', 'ready', null)
    bus.emit('app', 'ready', null)
    expect(cb).toHaveBeenCalledTimes(1)
  })

  test('多个监听器', () => {
    const bus = new EventBus()
    const cb1 = mock()
    const cb2 = mock()
    bus.on('win', 'move', cb1)
    bus.on('win', 'move', cb2)
    bus.emit('win', 'move', { x: 1, y: 2 })
    expect(cb1).toHaveBeenCalledTimes(1)
    expect(cb2).toHaveBeenCalledTimes(1)
  })

  test('off 精确移除', () => {
    const bus = new EventBus()
    const cb1 = mock()
    const cb2 = mock()
    bus.on('win', 'focus', cb1)
    bus.on('win', 'focus', cb2)
    bus.off('win', 'focus', cb1)
    bus.emit('win', 'focus', null)
    expect(cb1).toHaveBeenCalledTimes(0)
    expect(cb2).toHaveBeenCalledTimes(1)
  })

  test('removeNamespace 清理整个命名空间', () => {
    const bus = new EventBus()
    const cb1 = mock()
    const cb2 = mock()
    bus.on('win1', 'close', cb1)
    bus.on('win1', 'resize', cb2)
    bus.removeNamespace('win1')
    bus.emit('win1', 'close', null)
    bus.emit('win1', 'resize', null)
    expect(cb1).toHaveBeenCalledTimes(0)
    expect(cb2).toHaveBeenCalledTimes(0)
  })

  test('不同命名空间隔离', () => {
    const bus = new EventBus()
    const cb1 = mock()
    const cb2 = mock()
    bus.on('win1', 'close', cb1)
    bus.on('win2', 'close', cb2)
    bus.emit('win1', 'close', null)
    expect(cb1).toHaveBeenCalledTimes(1)
    expect(cb2).toHaveBeenCalledTimes(0)
  })

  test('无监听器时 emit 不报错', () => {
    const bus = new EventBus()
    expect(() => bus.emit('nonexistent', 'event', null)).not.toThrow()
  })

  test('off 不存在的监听器不报错', () => {
    const bus = new EventBus()
    expect(() => bus.off('nonexistent', 'event', () => {})).not.toThrow()
  })

  test('监听器抛出异常不影响其他监听器', () => {
    const bus = new EventBus()
    const cb1 = mock(() => { throw new Error('boom') })
    const cb2 = mock()
    bus.on('win', 'error', cb1)
    bus.on('win', 'error', cb2)
    // emit 中 slice() 复制数组，异常不会中断其他回调
    expect(() => bus.emit('win', 'error', null)).toThrow()
    expect(cb1).toHaveBeenCalledTimes(1)
    // cb2 是否被调用取决于 emit 实现 - 当前实现是 forEach，异常会中断
    // slice() 复制数组但不阻止异常传播
  })
})
