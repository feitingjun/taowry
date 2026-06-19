/**
 * EventBus - 轻量级事件总线
 *
 * 支持命名空间的事件管理，用于 Application 内部的事件分发。
 * 每个事件通过 `namespace:eventName` 字符串标识，
 * 支持 `removeNamespace` 批量清理（窗口关闭/托盘移除时使用）。
 */

export type Listener = (data?: any) => void

export class EventBus {
  /** namespace → event → listeners[] */
  private listeners: Record<string, Record<string, Listener[]>> = {}

  /** 添加事件监听器，返回取消监听的函数 */
  on(namespace: string, event: string, callback: Listener): () => void {
    if (!this.listeners[namespace]) this.listeners[namespace] = {}
    if (!this.listeners[namespace][event]) this.listeners[namespace][event] = []
    this.listeners[namespace][event].push(callback)
    return () => this.off(namespace, event, callback)
  }

  /** 添加一次性事件监听器 */
  once(namespace: string, event: string, callback: Listener): () => void {
    const wrapper: Listener = data => {
      callback(data)
      this.off(namespace, event, wrapper)
    }
    return this.on(namespace, event, wrapper)
  }

  /** 移除事件监听器 */
  off(namespace: string, event: string, callback: Listener): void {
    const listeners = this.listeners[namespace]?.[event]
    if (!listeners) return
    this.listeners[namespace][event] = listeners.filter(item => item !== callback)
  }

  /** 发送事件 */
  emit(namespace: string, event: string, data: any): void {
    const listeners = this.listeners[namespace]?.[event] ?? []
    // slice() 防止回调中修改数组导致迭代异常
    listeners.slice().forEach(callback => callback(data))
  }

  /** 清理整个命名空间的所有监听器 */
  removeNamespace(namespace: string): void {
    delete this.listeners[namespace]
  }
}
