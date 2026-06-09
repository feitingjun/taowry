import { platform } from 'process'
import { existsSync } from 'fs'
import { join, dirname } from 'path'
import { randomUUID } from 'crypto'

/** 二进制文件名 */
const BINARY_NAME = 'taowry'
const suffix = platform === 'win32' ? '.exe' : ''
const filename = BINARY_NAME + suffix

/** 生成一个唯一id */
export const uid = () => randomUUID()

/**
 * 获取 Rust 二进制文件路径
 *
 * @param userDir 用户指定的二进制目录（来自 ApplicationOptions.binaryDir）
 *
 * 查找顺序：
 * 1. userDir（用户显式指定）
 * 2. 项目根目录/.binary/（postinstall 下载位置）
 * 3. process.execPath 旁（bun build --compile 打包后）
 * 4. process.cwd()（兜底）
 */
export const getBinaryPath = (userDir?: string): string => {
  const candidates: string[] = []

  // 1. 用户显式指定的目录
  if (userDir) candidates.push(join(userDir, filename))

  // 2. 项目根目录/.binary/（postinstall 统一下载位置）
  candidates.push(join(process.cwd(), '.binary', filename))

  // 3. process.execPath 旁（打包后）
  candidates.push(join(dirname(process.execPath), filename))

  // 4. cwd 兜底
  candidates.push(join(process.cwd(), filename))

  for (const p of candidates) {
    if (existsSync(p)) return p
  }

  throw new Error(
    `[taowry] 找不到二进制文件，已检查:\n` +
    candidates.map(p => `  - ${p}`).join('\n') + '\n' +
    `请通过 ApplicationOptions.binaryDir 指定目录，或将 taowry 拷贝到以上任一位置`
  )
}
