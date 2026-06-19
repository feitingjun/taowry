/**
 * RPC bridge script 消息解析测试
 * 测试 window.ts 中注入的 __taowry bridge 的 IPC 消息解析逻辑
 */
import { describe, test, expect } from 'bun:test'

/** 模拟 RPC bridge 的消息处理逻辑（从 window.ts RPC_BRIDGE_SCRIPT 提取） */
function parseIpcMessage(body: string): { type: string; id?: number; method?: string; event?: string; data?: any; error?: string } | null {
  try {
    return JSON.parse(body)
  } catch {
    return null
  }
}

function classifyMessage(msg: ReturnType<typeof parseIpcMessage>): 'req' | 'res' | 'msg' | 'invalid' {
  if (!msg) return 'invalid'
  if (msg.type === 'req' && typeof msg.id === 'number' && typeof msg.method === 'string') return 'req'
  if (msg.type === 'res' && typeof msg.id === 'number') return 'res'
  if (msg.type === 'msg' && typeof msg.event === 'string') return 'msg'
  return 'invalid'
}

describe('RPC IPC message parsing', () => {
  test('解析 request 消息', () => {
    const msg = parseIpcMessage(JSON.stringify({ type: 'req', id: 1, method: 'echo', data: { msg: 'hi' } }))
    expect(msg).not.toBeNull()
    expect(classifyMessage(msg)).toBe('req')
    expect(msg!.id).toBe(1)
    expect(msg!.method).toBe('echo')
  })

  test('解析 response 消息', () => {
    const msg = parseIpcMessage(JSON.stringify({ type: 'res', id: 42, data: { result: 'ok' } }))
    expect(classifyMessage(msg)).toBe('res')
    expect(msg!.id).toBe(42)
  })

  test('解析 response 错误消息', () => {
    const msg = parseIpcMessage(JSON.stringify({ type: 'res', id: 3, error: 'something went wrong' }))
    expect(classifyMessage(msg)).toBe('res')
    expect(msg!.error).toBe('something went wrong')
  })

  test('解析 send 消息', () => {
    const msg = parseIpcMessage(JSON.stringify({ type: 'msg', event: 'update', data: { x: 1 } }))
    expect(classifyMessage(msg)).toBe('msg')
    expect(msg!.event).toBe('update')
  })

  test('无效 JSON 返回 null', () => {
    expect(parseIpcMessage('not json')).toBeNull()
  })

  test('缺少 type 字段视为 invalid', () => {
    expect(classifyMessage(parseIpcMessage(JSON.stringify({ id: 1 })))).toBe('invalid')
  })

  test('req 缺少 id 视为 invalid', () => {
    expect(classifyMessage(parseIpcMessage(JSON.stringify({ type: 'req', method: 'x' })))).toBe('invalid')
  })

  test('msg 缺少 event 视为 invalid', () => {
    expect(classifyMessage(parseIpcMessage(JSON.stringify({ type: 'msg', data: {} })))).toBe('invalid')
  })
})
