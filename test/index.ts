import { serve } from 'bun'
import { Application, BrowserWindow, defineRPC, type RPCSchema, Tray } from '../src/ts/index'
import h from './index.html'

const server = serve({
  port: 3000,
  routes: { '/': h }
})

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

const app = new Application()

const rpc = defineRPC<RPC>({
  requests: {
    echo: async data => {
      console.log(data.msg)
      // win.rpc?.messages.update({ message: 'host-reply: ' + data.msg })
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
console.log(h, 11111111)
const win = new BrowserWindow<RPC>('main', {
  title: '测试',
  width: 600,
  height: 400,
  url: 'http://localhost:3000/',
  rpc: rpc,
  devtools: true
  // enabledButtons: ['close']
})
win.openDevtools()

app.run()
