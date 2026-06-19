/**
 * RPC 通信共享类型定义
 *
 * 被 types.ts（Host 端 TypeScript API）和 client.ts（WebView 端 SDK）
 * 通过 `import type` 共享使用，避免重复定义。
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

/** RPC 方法类型转换 — 将接口中的方法签名转为实际的调用签名 */
export type RPCPromise<T, K extends PropertyKey> = T extends object
  ? K extends keyof T
    ? T[K] extends object
      ? K extends 'messages'
        ? {
            [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => any ? (...args: A) => void : never
          }
        : {
            [K2 in keyof T[K]]: T[K][K2] extends (...args: infer A) => infer R
              ? (...args: A) => Promise<Awaited<R>>
              : never
          }
      : {}
    : {}
  : {}

/** RPC 定义辅助类型（用于 defineRPC 参数类型推断） */
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

/** defineRPC 参数类型 */
export type DefineRPCConfig<T> = Omit<T, 'messages'> & {
  messages?: 'messages' extends keyof T
    ? T['messages'] extends object
      ? { [K in keyof T['messages']]?: T['messages'][K] }
      : T['messages']
    : never
}
