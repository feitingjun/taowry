// Minimal IPC test - checks if wry IPC handler works
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

  let html = r#"<html><body>
    <h1>IPC Test</h1>
    <button onclick="testIpc()">Test IPC</button>
    <div id="result"></div>
    <script>
      function testIpc() {
        try {
          window.ipc.postMessage('hello_from_webview');
          document.getElementById('result').textContent = 'IPC sent!';
        } catch(e) {
          document.getElementById('result').textContent = 'Error: ' + e.message;
        }
      }
      // Auto test
      setTimeout(function() { testIpc(); }, 1000);
    </script>
  </body></html>"#;

  let _webview = WebViewBuilder::new()
    .with_html(html)
    .with_ipc_handler(|request| {
      eprintln!("[IPC HANDLER] uri={}, body={}", request.uri(), request.body());
      println!("[IPC HANDLER] uri={}, body={}", request.uri(), request.body());
    })
    .build_as_child(&window)?;

  println!("Window created, waiting for IPC...");
  eprintln!("Window created, waiting for IPC...");

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
