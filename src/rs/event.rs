//! 窗口事件处理模块
//!
//! 将 tao 窗口事件转换为 IPC 消息发送给 Node.js 端，
//! 包括移动、缩放、焦点、光标、主题等事件。

use crate::application::Application;
use crate::listen::send_window_event;
use serde_json::{json, Value};
use tao::event::{
  ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Theme, WindowId};

/// 处理 tao 窗口事件，将其转换为 IPC 消息发送
pub fn handle_window_event(
  app: &mut Application,
  event_loop: &EventLoopWindowTarget<crate::application::Action>,
  window_id: WindowId,
  event: WindowEvent,
) {
  let Some(label) = app.label_for_window_id(window_id) else {
    return;
  };

  match event {
    WindowEvent::CloseRequested => {
      send_window_event(&label, "close", Value::Null);
      app.close_window(&label);
    }
    WindowEvent::Destroyed => {
      send_window_event(&label, "destroy", Value::Null);
    }
    WindowEvent::Moved(position) => {
      send_window_event(&label, "move", json!({ "x": position.x, "y": position.y }));
    }
    WindowEvent::Resized(size) => {
      if let Some(window) = app.get_window(&label) {
        let _ = window.resize_webview(tao::dpi::Size::Physical(size));
      }
      send_window_event(
        &label,
        "resize",
        json!({ "width": size.width, "height": size.height }),
      );
    }
    WindowEvent::Focused(focused) => {
      send_window_event(
        &label,
        if focused { "focus" } else { "blur" },
        Value::Null,
      );
    }
    WindowEvent::CursorMoved { position, .. } => {
      send_window_event(&label, "cursorMove", json!({ "x": position.x, "y": position.y }));
    }
    WindowEvent::CursorEntered { .. } => {
      send_window_event(&label, "cursorEnter", Value::Null);
    }
    WindowEvent::CursorLeft { .. } => {
      send_window_event(&label, "cursorOut", Value::Null);
    }
    WindowEvent::ThemeChanged(theme) => {
      send_window_event(&label, "theme", Value::String(theme_to_string(theme).to_string()));
    }
    WindowEvent::DroppedFile(path) => {
      send_window_event(
        &label,
        "droppedFile",
        json!({ "path": path.to_string_lossy().to_string() }),
      );
    }
    WindowEvent::HoveredFile(path) => {
      send_window_event(
        &label,
        "hoveredFile",
        json!({ "path": path.to_string_lossy().to_string() }),
      );
    }
    WindowEvent::HoveredFileCancelled => {
      send_window_event(&label, "hoveredFileCancelled", Value::Null);
    }
    WindowEvent::ReceivedImeText(text) => {
      send_window_event(&label, "receivedImeText", Value::String(text));
    }
    WindowEvent::KeyboardInput {
      event,
      is_synthetic,
      ..
    } => {
      send_window_event(
        &label,
        "keyboardInput",
        json!({
          "state": format!("{:?}", event.state),
          "key": format!("{:?}", event.logical_key),
          "physicalKey": format!("{:?}", event.physical_key),
          "repeat": event.repeat,
          "synthetic": is_synthetic
        }),
      );
    }
    WindowEvent::ModifiersChanged(modifiers) => {
      send_window_event(
        &label,
        "modifiersChanged",
        json!({
          "shift": modifiers.shift_key(),
          "control": modifiers.control_key(),
          "alt": modifiers.alt_key(),
          "super": modifiers.super_key()
        }),
      );
    }
    WindowEvent::MouseWheel { delta, phase, .. } => {
      send_window_event(
        &label,
        "mouseWheel",
        json!({
          "delta": mouse_scroll_delta(delta),
          "phase": touch_phase(phase)
        }),
      );
    }
    WindowEvent::MouseInput { state, button, .. } => {
      send_window_event(
        &label,
        "mouseInput",
        json!({
          "state": element_state(state),
          "button": mouse_button(button)
        }),
      );
    }
    WindowEvent::TouchpadPressure {
      pressure, stage, ..
    } => {
      send_window_event(
        &label,
        "touchpadPressure",
        json!({ "pressure": pressure, "stage": stage }),
      );
    }
    WindowEvent::AxisMotion { axis, value, .. } => {
      send_window_event(
        &label,
        "axisMotion",
        json!({ "axis": format!("{:?}", axis), "value": value }),
      );
    }
    WindowEvent::Touch(touch) => {
      send_window_event(
        &label,
        "touch",
        json!({
          "phase": touch_phase(touch.phase),
          "location": { "x": touch.location.x, "y": touch.location.y },
          "force": format!("{:?}", touch.force),
          "id": touch.id
        }),
      );
    }
    WindowEvent::ScaleFactorChanged {
      scale_factor,
      new_inner_size,
    } => {
      if let Some(window) = app.get_window(&label) {
        let _ = window.resize_webview(tao::dpi::Size::Physical(*new_inner_size));
      }
      send_window_event(
        &label,
        "scaleFactorChanged",
        json!({
          "scaleFactor": scale_factor,
          "innerSize": {
            "width": new_inner_size.width,
            "height": new_inner_size.height
          }
        }),
      );
    }
    WindowEvent::DecorationsClick => {
      send_window_event(&label, "decorationsClick", Value::Null);
    }
    _ => {
      let _ = event_loop;
    }
  }
}

fn theme_to_string(theme: Theme) -> &'static str {
  match theme {
    Theme::Light => "light",
    Theme::Dark => "dark",
    _ => "light",
  }
}

fn element_state(state: ElementState) -> &'static str {
  match state {
    ElementState::Pressed => "pressed",
    ElementState::Released => "released",
    _ => "unknown",
  }
}

fn mouse_button(button: MouseButton) -> String {
  match button {
    MouseButton::Left => "left".to_string(),
    MouseButton::Right => "right".to_string(),
    MouseButton::Middle => "middle".to_string(),
    MouseButton::Other(value) => format!("other:{}", value),
    _ => "unknown".to_string(),
  }
}

fn touch_phase(phase: TouchPhase) -> &'static str {
  match phase {
    TouchPhase::Started => "started",
    TouchPhase::Moved => "moved",
    TouchPhase::Ended => "ended",
    TouchPhase::Cancelled => "cancelled",
    _ => "unknown",
  }
}

fn mouse_scroll_delta(delta: MouseScrollDelta) -> Value {
  match delta {
    MouseScrollDelta::LineDelta(x, y) => json!({ "type": "line", "x": x, "y": y }),
    MouseScrollDelta::PixelDelta(position) => {
      json!({ "type": "pixel", "x": position.x, "y": position.y })
    }
    _ => json!({ "type": "unknown" }),
  }
}
