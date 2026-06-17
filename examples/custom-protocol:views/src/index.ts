import { Application, BrowserWindow } from '../../../src/ts'
import assets from '../assets'

const app = new Application({
  protocol: request => {
    const path = new URL(request.url).pathname
    const asset = assets.find(asset => asset.path === path)
    return asset ? new Response(asset.content, { headers: { 'Content-Type': asset.type } }) : new Response()
  }
})
await app.whenReady()

const win = new BrowserWindow('main', {
  url: `views://localhost/index.html`,
  width: 600,
  height: 400,
  title: 'My App',
  devtools: true
})

win.on('close', () => app.quit())
