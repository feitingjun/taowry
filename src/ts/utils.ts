import { platform } from 'process'
import { existsSync } from 'fs'
import { join } from 'path'
import { randomUUID } from 'crypto'

/** 二进制文件名 */
const BINARY_NAME = 'node-webview'
const suffix = platform === 'win32' ? '.exe' : ''

/** 生成一个唯一id */
export const uid = () => randomUUID()

/**
 * 获取二进制文件路径
 * 二进制文件由 postinstall 脚本在 npm install 时下载
 * 本地开发时通过 npm run build 编译
 */
export const getBinaryPath = (): string => {
  const filename = BINARY_NAME + suffix
  const path = join(__dirname, filename)
  if (existsSync(path)) return path
  throw new Error(
    `[node-webview] 找不到二进制文件: ${path}\n` +
    `请运行 npm run build (本地开发) 或确认 postinstall 脚本正常执行 (npm install)`
  )
}
