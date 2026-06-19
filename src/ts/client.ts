/**
 * taowry/client - WebView 端客户端 SDK
 * 通过 taowry/client 子路径导入
 * 用于与 taowry Host 端进行双向 RPC 通信
 */

import type { RPCInterface, RPCPromise, RPCSchema } from './rpc-types.js'
import type { DefineRPCConfig } from './rpc-types.js'

export type { RPCInterface, RPCPromise, RPCSchema }

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

// ===== 全局声明（window.__taowry） =====

declare global {
  interface Window {
    __taowry?: {
      defineRPC(config: {
        requests?: Record<string, (data: any) => any>
        messages?: Record<string, ((data: any) => void) | undefined>
      }): {
        requests: Record<string, (data: any) => Promise<any>>
        messages: Record<string, (data: any) => void>
        on(event: string, callback: (data: any) => void): () => void
        off(event: string, callback: (data: any) => void): void
      }
    }
    ipc?: {
      postMessage(message: string): void
    }
  }
}

// ===== 导出 API =====

/**
 * 创建类型安全的 WebView 端 RPC 实例
 *
 * @example
 * ```typescript
 * import { defineRPC, RPCInterface } from 'taowry/client'
 *
 * interface MyRPC extends RPCInterface {
 *   host: {
 *     requests: {
 *       echo: (data: { msg: string }) => { received: string }
 *     }
 *     messages: {
 *       update: { message: string }
 *     }
 *   }
 *   webview: {
 *     requests: {
 *       renderData: (data: { items: string[] }) => { count: number }
 *     }
 *     messages: {
 *       ready: { url: string }
 *     }
 *   }
 * }
 *
 * const rpc = defineRPC<MyRPC>({
 *   requests: {
 *     renderData: (data) => ({ count: data.items.length })
 *   },
 *   messages: {
 *     ready: (data) => console.log('就绪:', data.url)
 *   }
 * })
 *
 * // 调用 Host 端方法
 * const result = await rpc.requests.echo({ msg: 'hello' })
 *
 * // 监听 Host 端发送的消息
 * rpc.on('update', (data) => console.log(data.message))
 *
 * // 向 Host 端发送消息（fire-and-forget，同步调用）
 * rpc.messages.ready({ url: location.href })
 * ```
 */
export function defineRPC<T extends RPCInterface>(
  config: DefineRPCConfig<T['webview']>
): WebviewRPCInstance<T> {
  if (!window.__taowry) {
    throw new Error('window.__taowry is not available. Make sure you are running inside a taowry WebView.')
  }
  const rawRpc = window.__taowry.defineRPC({
    requests: ((config as any)?.requests ?? {}) as Record<string, (data: any) => any>,
    messages: (config?.messages ?? {}) as Record<string, ((data: any) => void) | undefined>
  })

  return rawRpc as unknown as WebviewRPCInstance<T>
}

