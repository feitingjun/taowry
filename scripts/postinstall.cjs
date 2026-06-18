/**
 * postinstall 脚本
 * 在 npm install 时自动下载对应平台的 .node 原生模块
 * 下载到当前 npm 包安装目录（node_modules/taowry/）
 */
const { platform, arch } = require('process')
const { existsSync, createWriteStream } = require('fs')
const { join } = require('path')
const https = require('https')

const REPO = 'feitingjun/taowry'
const BASE_URL = `https://github.com/${REPO}/releases/latest/download`

/** 获取 napi-rs 目标三元组 */
function getTarget() {
  let sys, cpu
  switch (platform) {
    case 'darwin': sys = 'apple-darwin'; break
    case 'win32': sys = 'pc-windows-msvc'; break
    case 'linux': sys = 'unknown-linux-gnu'; break
    default: sys = 'unknown-unknown'
  }
  switch (arch) {
    case 'x64': cpu = 'x86_64'; break
    case 'arm64': cpu = 'aarch64'; break
    default: cpu = 'x86_64'
  }
  return `${cpu}-${sys}`
}

/** 下载文件（跟随重定向） */
function download(url) {
  return new Promise((resolve, reject) => {
    https.get(url, (res) => {
      if (res.statusCode === 200) {
        resolve(res)
      } else if ([301, 302, 303, 307, 308].includes(res.statusCode) && res.headers.location) {
        download(res.headers.location).then(resolve).catch(reject)
      } else {
        reject(new Error(`下载失败, HTTP ${res.statusCode}`))
      }
    }).on('error', reject)
  })
}

async function main() {
  const target = getTarget()
  const remoteFile = `taowry.${target}.node`
  const localFile = 'taowry.node'

  // 目标目录: npm 包安装目录（__dirname = node_modules/taowry/scripts/）
  const pkgDir = join(__dirname, '..')
  const outPath = join(pkgDir, localFile)

  // 如果已存在则跳过
  if (existsSync(outPath)) {
    console.log(`\x1b[32m[taowry] native 模块已存在: ${outPath}\x1b[0m`)
    return
  }

  const url = `${BASE_URL}/${remoteFile}`
  console.log(`\x1b[32m[taowry] 正在下载 native 模块...\x1b[0m`)
  console.log(`\x1b[32m[taowry] 平台: ${platform}-${arch} (${target})\x1b[0m`)
  console.log(`\x1b[32m[taowry] URL: ${url}\x1b[0m`)

  try {
    const res = await download(url)
    await new Promise((resolve, reject) => {
      const stream = createWriteStream(outPath)
      res.pipe(stream)
      stream.on('finish', () => { stream.close(); resolve() })
      stream.on('error', reject)
      res.on('error', reject)
    })
    console.log(`\x1b[32m[taowry] 下载完成: ${outPath}\x1b[0m`)
  } catch (err) {
    console.error(`\x1b[33m[taowry] 下载失败: ${err.message}\x1b[0m`)
    console.error(`\x1b[33m[taowry] 如果是本地开发，请手动运行 npm run build\x1b[0m`)
    // postinstall 失败不阻塞安装
  }
}

main()
