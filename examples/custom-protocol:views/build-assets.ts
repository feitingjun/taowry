import { build } from 'bun'

// 1. 构建前端资源
const result = await build({
  entrypoints: ['frontend/index.html'],
  target: 'browser'
})

const mimeMap: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.css': 'text/css',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.json': 'application/json'
}

const entries: string[] = []
for (const output of result.outputs) {
  const text = await output.text()
  const ext = output.path.match(/\.\w+$/)?.[0] || ''
  const mime = mimeMap[ext] || 'application/octet-stream'
  const cleanPath = '/' + output.path.replace(/^\.\//, '')
  entries.push(
    `{path:${JSON.stringify(cleanPath)},type:${JSON.stringify(mime)},content:${JSON.stringify(text)}}`
  )
}

// 2. 生成 assets.ts
const assetsCode = `// Auto-generated — DO NOT EDIT
export default [${entries.join(',')}];
`
const assetsPath = 'assets.ts'
await Bun.write(assetsPath, assetsCode)
console.log(`✓ Bundled ${result.outputs.length} assets`)
