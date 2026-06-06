import { createRoot } from 'react-dom/client'
import { defineRPC } from '../src/ts/client'
import type { RPC } from './index'

const rpc = defineRPC<RPC>({
  requests: {
    update: async data => {
      return { received: data.msg }
    }
  },
  messages: {
    update: async data => {
      debugger
    }
  }
})

rpc.on('update', async data => {
  debugger
})

const App = () => {
  return (
    <div>
      <h1>hello world</h1>
      <button
        onClick={async () => {
          await rpc.messages.update({ message: '11111111' })
          console.log('[webview] message sent, now debugger...')
          debugger
        }}
      >
        点击
      </button>
    </div>
  )
}

const root = createRoot(document.getElementById('root')!)
root.render(<App />)
