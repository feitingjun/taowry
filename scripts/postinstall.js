/**
 * postinstall 脚本
 * 在 npm install 时自动下载对应平台的二进制文件
 * 确保打包工具(pkg/bun build等)可以将二进制文件打包进去
 */
const { platform, arch } = require('process')
const { existsSync, createWriteStream, mkdirSync } = require('fs')
const { join } = require('path')
const https = require('https')

const REPO = 'feitingjun/node-webview'
const BASE_URL = `https://github.com/${REPO}/releases/latest/download`

// 检测目标平台三元组
function getTarget() {
  let sys, cpu
  switch (platform) {
    case 'darwin': sys = 'apple-darwin'; break
    case 'win32': sys = 'pc-windows-gnu'; break
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

// 获取输出文件名
function getBinaryName() {
  const suffix = platform === 'win32' ? '.exe' : ''
  return `node-webview${suffix}`
}

// 跟随重定向下载
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
  const suffix = platform === 'win32' ? '.exe' : ''
  const remoteFile = `${target}${suffix}`
  const localFile = getBinaryName()

  // 目标目录: 优先 dist/(消费者安装场景), 否则 src/ts/(本地开发)
  const distDir = join(__dirname, '..', 'dist')
  const srcTsDir = join(__dirname, '..', 'src', 'ts')
  const outDir = existsSync(distDir) ? distDir : srcTsDir

  if (!existsSync(outDir)) {
    mkdirSync(outDir, { recursive: true })
  }

  const outPath = join(outDir, localFile)

  // 如果已存在则跳过
  if (existsSync(outPath)) {
    console.log(`\x1b[32m[node-webview] 二进制文件已存在: ${outPath}\x1b[0m`)
    return
  }

  const url = `${BASE_URL}/${remoteFile}`
  console.log(`\x1b[32m[node-webview] 正在下载二进制文件...\x1b[0m`)
  console.log(`\x1b[32m[node-webview] 平台: ${target}\x1b[0m`)
  console.log(`\x1b[32m[node-webview] URL: ${url}\x1b[0m`)

  try {
    const res = await download(url)
    await new Promise((resolve, reject) => {
      const stream = createWriteStream(outPath, { mode: 0o755 })
      res.pipe(stream)
      stream.on('finish', () => { stream.close(); resolve() })
      stream.on('error', reject)
      res.on('error', reject)
    })
    console.log(`\x1b[32m[node-webview] 下载完成: ${outPath}\x1b[0m`)
  } catch (err) {
    console.error(`\x1b[33m[node-webview] 下载失败: ${err.message}\x1b[0m`)
    console.error(`\x1b[33m[node-webview] 如果是本地开发，请手动运行 npm run build\x1b[0m`)
    // postinstall 失败不阻塞安装(本地开发场景)
  }
}

main()
