// IPC test with file:// URL
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

  let test_dir = std::env::current_dir().unwrap().join("test");
  let url = format!("file:{}/ipc_file_test.html", test_dir.display());
  eprintln!("Loading URL: {}", url);

  let init_script = r#"
(function() {
  console.log('Init script running');
  window.__testInit = true;
})();
"#;

  let _webview = WebViewBuilder::new()
    .with_url(&url)
    .with_initialization_script(init_script)
    .with_ipc_handler(|request| {
      eprintln!("[IPC HANDLER] uri={}, body={}", request.uri(), request.body());
    })
    .build_as_child(&window)?;

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
