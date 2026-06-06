// IPC test - load file as HTML string
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

  // Read file content
  let test_file = std::env::current_dir().unwrap().join("test/ipc_file_test.html");
  let html = std::fs::read_to_string(&test_file).unwrap();
  eprintln!("Loading HTML from file (not using file:// URL)");

  let _webview = WebViewBuilder::new()
    .with_html(&html)
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
