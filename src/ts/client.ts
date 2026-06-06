/**
 * node-webview/client - WebView 端客户端 SDK
 * 通过 node-webview/client 子路径导入
 * 用于与 node-webview Host 端进行双向 RPC 通信
 */

/** RPC 接口定义 — 分别定义 host 端和 webview 端的方法 */
export interface RPCInterface {
  host?: {
    /** 请求方法（request-response，webview 调用 host） */
    requests?: Record<string, (...args: any[]) => any>
    /** 消息（fire-and-forget，双向） */
    messages?: Record<string, (...args: any[]) => void>
  }
  webview?: {
    /** 请求方法（request-response，host 调用 webview） */
    requests?: Record<string, (...args: any[]) => any>
    /** 消息（fire-and-forget，双向） */
    messages?: Record<string, (...args: any[]) => void>
  }
}

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

// ===== 全局声明（window.__nodeWebview） =====

declare global {
  interface Window {
    __nodeWebview?: {
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
 * import { defineRPC, RPCInterface } from 'node-webview/client'
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
  if (!window.__nodeWebview) {
    throw new Error(
      'window.__nodeWebview is not available. Make sure you are running inside a node-webview WebView.'
    )
  }
  const rawRpc = window.__nodeWebview.defineRPC({
    requests: ((config as any)?.requests ?? {}) as Record<string, (data: any) => any>,
    messages: (config?.messages ?? {}) as Record<string, ((data: any) => void) | undefined>
  })

  return rawRpc as unknown as WebviewRPCInstance<T>
}

export type RPCPromise<T, K extends PropertyKey> = T extends object
  ? K extends keyof T
    ? T[K] extends object
      ? K extends 'messages'
        ? {
            [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => any
              ? (...args: A) => void
              : never
          }
        : {
            [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => infer R
              ? (...args: A) => Promise<Awaited<R>>
              : never
          }
      : {}
    : {}
  : {}

/**RPC定义辅助类型 */
export type RPCSchema<
  T extends {
    requests?: Record<string, (...args: any[]) => any>
    messages?: Record<string, any>
  }
> = {
  [K in keyof T]: K extends 'messages'
    ? {
        [K2 in keyof T[K]]: (data: T[K][K2]) => void
      }
    : {
        [K2 in keyof T[K]]: T[K][K2]
      }
}

/**defineRPC参数类型 */
type DefineRPCConfig<T> = Omit<T, 'messages'> & {
  messages?: 'messages' extends keyof T
    ? T['messages'] extends object
      ? { [K in keyof T['messages']]?: T['messages'][K] }
      : T['messages']
    : never
}
