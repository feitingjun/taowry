// IPC test with initialization script
use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_inner_size(tao::dpi::LogicalSize::new(600.0, 400.0))
    .build(&event_loop)
    .unwrap();

  // Same init script as node-webview RPC bridge
  let init_script = r#"
(function() {
  if (window.__nodeWebview) return;
  var counter = 0;
  var callbacks = {};
  window.__nodeWebview = {
    invoke: function(method, data) {
      return new Promise(function(resolve, reject) {
        var id = ++counter;
        callbacks[id] = { resolve: resolve, reject: reject };
        window.ipc.postMessage(JSON.stringify({ __rpc: true, id: id, method: method, data: data }));
      });
    },
    _resolve: function(id, data, error) {
      var cb = callbacks[id];
      if (!cb) return;
      delete callbacks[id];
      if (error) cb.reject(new Error(error));
      else cb.resolve(data);
    }
  };
})();
"#;

  let html = r#"<html><body>
    <h1>IPC Test with Init Script</h1>
    <div id="result"></div>
    <script>
      setTimeout(function() {
        try {
          window.ipc.postMessage('test_direct');
          document.getElementById('result').textContent = 'IPC sent! hasNodeWebview=' + !!window.__nodeWebview;
        } catch(e) {
          document.getElementById('result').textContent = 'Error: ' + e.message;
        }
      }, 1000);
    </script>
  </body></html>"#;

  let _webview = WebViewBuilder::new()
    .with_html(html)
    .with_initialization_script(init_script)
    .with_ipc_handler(|request| {
      eprintln!("[IPC HANDLER] uri={}, body={}", request.uri(), request.body());
    })
    .build_as_child(&window)?;

  println!("Window created, waiting for IPC...");

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    if let Event::WindowEvent {
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      *control_flow = ControlFlow::Exit;
    }
  });
}
