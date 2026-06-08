import { build } from 'bun'
import {
  Application,
  BrowserWindow,
  defineRPC,
  type RPCSchema,
  type ProtocolRequest,
  type ProtocolResponse,
  Tray
} from '../src/ts/index'

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

const b = await build({
  entrypoints: [process.cwd() + '/test/index.html'],
  target: 'browser'
})

const win = new BrowserWindow<RPC>('main', {
  title: '测试',
  width: 600,
  height: 400,
  url: 'views://index.html',
  rpc: rpc,
  devtools: true,
  protocolHandler: async (request: ProtocolRequest): Promise<ProtocolResponse> => {
    let path = request.uri.replace('views://index.html', '')
    let file
    if (path === '/') {
      file = b.outputs.find(o => o.path.endsWith('index.html'))
    } else {
      file = b.outputs.find(o => o.path.endsWith(path))
    }
    return {
      data: (await file?.text()) ?? '',
      mimeType: file?.type,
      statusCode: 200
    }
  }
})

app.run()
