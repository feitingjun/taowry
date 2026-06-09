import { build } from 'bun'
import { Application, BrowserWindow, defineRPC, type RPCSchema, Tray } from '../src/ts/index'
import binary from '../.binary/taowry' with { type: 'file' }

export type RPC = {
  host: RPCSchema<{
    requests: {
      echo: (data: { msg: string }) => { received: string }
    }
    messages: {
      update: { message: string }
    }
  }>
  webview: RPCSchema<{
    requests: {
      update: (data: { msg: string }) => { received: string }
    }
    messages: {
      update: { message: string }
    }
  }>
}

const b = await build({
  entrypoints: [process.cwd() + '/test/index.html'],
  target: 'browser'
})

const app = new Application({
  protocol: async request => {
    const filename = new URL(request.url).pathname.replace(/^\/+/, './')
    const file = b.outputs.find(o => o.path === filename)
    if (file) {
      return new Response(await file.text(), {
        status: 200,
        headers: { 'Content-Type': file.type ?? 'application/octet-stream' }
      })
    }
    return new Response('Not Found', { status: 404 })
  }
})

const rpc = defineRPC<RPC>({
  requests: {
    echo: async data => {
      console.log(data.msg)
      return { received: data.msg }
    }
  },
  messages: {
    update: async data => {
      console.log('收到消息:', data.message)
      win.setResizable(false)
    }
  }
})

const win = new BrowserWindow<RPC>('main', {
  title: '测试',
  width: 600,
  height: 400,
  url: 'views://app/index.html',
  rpc: rpc,
  devtools: true
})

app.run()
