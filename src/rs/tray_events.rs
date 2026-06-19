//! 托盘事件处理模块
//!
//! 将 tray-icon 事件转换为 JSON 数据，供 IPC 通道发送。

use serde_json::{json, Value};
use tray_icon::TrayIconEvent;

/// 将 TrayIconEvent 转换为 (method, data) 对
pub(crate) fn tray_event_payload(event: TrayIconEvent) -> (&'static str, Value) {
    match event {
        TrayIconEvent::Click {
            position,
            rect,
            button,
            button_state,
            ..
        } => (
            "click",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect),
              "button": format!("{:?}", button),
              "state": format!("{:?}", button_state)
            }),
        ),
        TrayIconEvent::DoubleClick {
            position,
            rect,
            button,
            ..
        } => (
            "doubleClick",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect),
              "button": format!("{:?}", button)
            }),
        ),
        TrayIconEvent::Enter { position, rect, .. } => (
            "enter",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect)
            }),
        ),
        TrayIconEvent::Move { position, rect, .. } => (
            "move",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect)
            }),
        ),
        TrayIconEvent::Leave { position, rect, .. } => (
            "leave",
            json!({
              "position": { "x": position.x, "y": position.y },
              "rect": tray_rect_payload(rect)
            }),
        ),
        _ => ("unknown", Value::Null),
    }
}

/// 将 tray_icon::Rect 转为 JSON
pub(crate) fn tray_rect_payload(rect: tray_icon::Rect) -> Value {
    json!({
      "x": rect.position.x,
      "y": rect.position.y,
      "width": rect.size.width,
      "height": rect.size.height
    })
}
