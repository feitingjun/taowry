/**
 * native-module.ts JSON 辅助函数单元测试
 */
import { describe, test, expect } from 'bun:test'
import { json, parse, parseOrNull } from '../native-module.js'

describe('json()', () => {
  test('序列化普通对象', () => {
    expect(json({ a: 1 })).toBe('{"a":1}')
  })

  test('null 序列化为字符串 "null"', () => {
    expect(json(null)).toBe('null')
  })

  test('undefined 序列化为字符串 "null"', () => {
    expect(json(undefined)).toBe('null')
  })

  test('数组序列化', () => {
    expect(json([1, 2, 3])).toBe('[1,2,3]')
  })
})

describe('parse()', () => {
  test('解析 JSON 对象', () => {
    expect(parse('{"a":1}')).toEqual({ a: 1 })
  })

  test('解析 JSON 数组', () => {
    expect(parse('[1,2,3]')).toEqual([1, 2, 3])
  })

  test('解析 null 文本', () => {
    expect(parse('null')).toBeNull()
  })
})

describe('parseOrNull()', () => {
  test('解析 JSON 对象', () => {
    expect(parseOrNull('{"a":1}')).toEqual({ a: 1 })
  })

  test('"null" 字符串返回 null', () => {
    expect(parseOrNull('null')).toBeNull()
  })
})
