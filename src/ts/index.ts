import Application, { getCurrentApplication } from './app'
import BrowserWindow from './window'
import { Tray } from './tray'
import { RPCInterface, DefineRPCConfig } from './types'

/** 通过标签名获取已创建的窗口实例 */
export const getWindow = <T extends RPCInterface = any>(label: string): BrowserWindow<T> | undefined => {
  return getCurrentApplication()?.getWindow(label)
}

/**
 * 创建类型安全的 Host 端 RPC 配置
 *
 * @example
 * ```typescript
 * import { defineRPC, RPCInterface, BrowserWindow } from 'node-webview'
 *
 * interface MyRPC extends RPCInterface {
 *   host: {
 *     requests: {
 *       getUserInfo: (data: { userId: string }) => { name: string }
 *     }
 *     messages: {
 *       pageReady: { url: string }
 *     }
 *   }
 *   webview: {
 *     requests: {
 *       renderData: (data: { items: string[] }) => { count: number }
 *     }
 *     messages: {
 *       userAction: { action: string }
 *     }
 *   }
 * }
 *
 * const rpcConfig = defineRPC<MyRPC>({
 *   requests: {
 *     getUserInfo: async (data) => ({ name: 'Alice' })
 *   },
 *   messages: {
 *     pageReady: (data) => console.log('页面就绪:', data.url)
 *   }
 * })
 *
 * const win = new BrowserWindow<MyRPC>('main', {
 *   url: 'https://example.com',
 *   rpc: rpcConfig
 * })
 *
 * // 调用 WebView 端方法
 * const result = await win.rpc.requests.renderData({ items: ['a', 'b'] })
 *
 * // 监听 WebView 发送的消息
 * win.rpc.on('userAction', (data) => console.log(data.action))
 *
 * // 向 WebView 发送消息
 * await win.rpc.messages.pageReady({ url: 'https://...' })
 * ```
 */
export function defineRPC<T extends RPCInterface>(config: DefineRPCConfig<T['host']>): T['host'] {
  return config
}

export { Application, BrowserWindow, BrowserWindow as Window, Tray }

export * from './types'
