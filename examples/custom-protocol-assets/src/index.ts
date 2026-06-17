import { Application, BrowserWindow } from '../../../src/ts'

// bun打包成二进制文件时需要将前端文件构建成一个独立HTML文件，否则存在问题
// 问题1：bun会将嵌入目录内的js文件放在dist目录外面，并且html引入的js路径会更改为虚拟目录的绝对路径，导致解析文件路径失败
// 问题2：bun嵌入的js文件会损坏导致不能使用
const root = process.env.NODE_ENV === 'development' ? process.cwd() : '/$bunfs/root'

const app = new Application({ assets: root + '/dist' })
await app.whenReady()

const win = new BrowserWindow('main', {
  url: `assets://index.html`,
  width: 600,
  height: 400,
  title: 'My App',
  devtools: true
})

win.on('close', () => app.quit())
