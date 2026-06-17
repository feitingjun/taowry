import { serve } from 'bun'
import { Application, BrowserWindow } from '../../../src/ts'
import html from '../frontend/index.html'

serve({
  port: 4000,
  routes: {
    '/': html
  }
})

const app = new Application()
await app.whenReady()

const win = new BrowserWindow('main', {
  url: `http://localhost:4000`,
  width: 600,
  height: 400,
  title: 'My App',
  devtools: true
})

win.on('close', () => app.quit())
