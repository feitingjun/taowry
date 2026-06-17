//! 窗口操作 napi 导出函数 — 直接执行

use napi::bindgen_prelude::*;
use serde_json::{json, Value};

use super::helpers::*;

/// 通过标签获取窗口引用并执行操作
fn with_window<F, R>(label: &str, f: F) -> Result<R>
where
    F: FnOnce(&crate::window::BrowserWindow) -> Result<R>,
{
    crate::with_app(|app| {
        let win = app
            .get_window(label)
            .ok_or_else(|| Error::from_reason(format!("window '{}' does not exist", label)))?;
        f(win)
    })
}

// ===== WebView 操作 =====

#[napi]
fn window_close(label: String) -> Result<()> {
    crate::with_app(|app| {
        app.close_window(&label);
        Ok(())
    })
}

#[napi]
fn window_request_redraw(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.request_redraw();
        Ok(())
    })
}

#[napi]
fn window_set_url(label: String, url: String) -> Result<()> {
    with_window(&label, |w| {
        w.set_url(&url)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_load_url_with_headers(label: String, data: String) -> Result<()> {
    let data = parse_json(&data);
    let url = data
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::from_reason("url is required"))?
        .to_string();
    let headers = headers_from_value(data.get("headers").unwrap_or(&Value::Null))
        .map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.load_url_with_headers(&url, headers)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_url(label: String) -> Result<String> {
    with_window(&label, |w| {
        w.url().map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_evaluate_script(label: String, script: String) -> Result<()> {
    with_window(&label, |w| {
        w.evaluate_script(&script)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_print(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.print().map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_open_devtools(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.open_devtools();
        Ok(())
    })
}

#[napi]
fn window_close_devtools(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.close_devtools();
        Ok(())
    })
}

#[napi]
fn window_is_devtools_open(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_devtools_open()))
}

#[napi]
fn window_zoom(label: String, scale: f64) -> Result<()> {
    with_window(&label, |w| {
        w.zoom(scale).map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_clear_all_browsing_data(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.clear_all_browsing_data()
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_set_background_color(label: String, data: String) -> Result<()> {
    let color = color_value(&parse_json(&data)).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.set_background_color(color)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_set_window_background_color(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    let color = if v.is_null() {
        None
    } else {
        Some(color_value(&v).map_err(Error::from_reason)?)
    };
    with_window(&label, |w| {
        w.set_window_background_color(color);
        Ok(())
    })
}

// ===== 尺寸/位置 =====

#[napi]
fn window_inner_position(label: String) -> Result<String> {
    with_window(&label, |w| {
        let p = w
            .inner_position()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(pos_to_json(p.x as f64, p.y as f64))
    })
}

#[napi]
fn window_outer_position(label: String) -> Result<String> {
    with_window(&label, |w| {
        let p = w
            .outer_position()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(pos_to_json(p.x as f64, p.y as f64))
    })
}

#[napi]
fn window_set_outer_position(label: String, data: String) -> Result<()> {
    let pos = position_from_value(&parse_json(&data))
        .ok_or_else(|| Error::from_reason("position is invalid"))?;
    with_window(&label, |w| {
        w.set_outer_position(pos);
        Ok(())
    })
}

#[napi]
fn window_inner_size(label: String) -> Result<String> {
    with_window(&label, |w| Ok(size_to_json(w.inner_size())))
}

#[napi]
fn window_set_inner_size(label: String, data: String) -> Result<String> {
    let size = size_value(&parse_json(&data)).map_err(Error::from_reason)?;
    with_window(&label, |w| Ok(size_to_json(w.set_inner_size(size))))
}

#[napi]
fn window_outer_size(label: String) -> Result<String> {
    with_window(&label, |w| Ok(size_to_json(w.outer_size())))
}

#[napi]
fn window_set_min_inner_size(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    with_window(&label, |w| {
        if v.is_null() {
            w.set_min_inner_size::<tao::dpi::Size>(None);
        } else {
            w.set_min_inner_size(Some(size_value(&v).map_err(Error::from_reason)?));
        }
        Ok(())
    })
}

#[napi]
fn window_set_max_inner_size(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    with_window(&label, |w| {
        if v.is_null() {
            w.set_max_inner_size::<tao::dpi::Size>(None);
        } else {
            w.set_max_inner_size(Some(size_value(&v).map_err(Error::from_reason)?));
        }
        Ok(())
    })
}

#[napi]
fn window_set_inner_size_constraints(label: String, data: String) -> Result<()> {
    let c = window_constraints_value(&parse_json(&data)).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.set_inner_size_constraints(c);
        Ok(())
    })
}

#[napi]
fn window_scale_factor(label: String) -> Result<f64> {
    with_window(&label, |w| Ok(w.scale_factor()))
}

#[napi]
fn window_request_user_attention(label: String, data: String) -> Result<()> {
    let attention = user_attention_value(&parse_json(&data)).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.request_user_attention(attention);
        Ok(())
    })
}

// ===== 窗口属性 =====

#[napi]
fn window_set_title(label: String, title: String) -> Result<()> {
    with_window(&label, |w| {
        w.set_title(&title);
        Ok(())
    })
}

#[napi]
fn window_title(label: String) -> Result<String> {
    with_window(&label, |w| Ok(w.title()))
}

#[napi]
fn window_set_visible(label: String, visible: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_visible(visible);
        Ok(())
    })
}

#[napi]
fn window_is_visible(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_visible()))
}

#[napi]
fn window_focus(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.focus_window();
        Ok(())
    })
}

#[napi]
fn window_has_focus(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.has_focus()))
}

#[napi]
fn window_set_resizable(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_resizable(value);
        Ok(())
    })
}

#[napi]
fn window_is_resizable(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_resizable()))
}

#[napi]
fn window_set_minimizable(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_minimizable(value);
        Ok(())
    })
}

#[napi]
fn window_is_minimizable(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_minimizable()))
}

#[napi]
fn window_set_maximizable(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_maximizable(value);
        Ok(())
    })
}

#[napi]
fn window_is_maximizable(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_maximizable()))
}

#[napi]
fn window_set_closable(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_closable(value);
        Ok(())
    })
}

#[napi]
fn window_is_closable(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_closable()))
}

#[napi]
fn window_set_enabled_buttons(label: String, data: String) -> Result<()> {
    let (c, m, x) = enabled_buttons_value(&parse_json(&data)).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.set_enabled_buttons(c, m, x);
        Ok(())
    })
}

#[napi]
fn window_enabled_buttons(label: String) -> Result<String> {
    with_window(&label, |w| {
        let buttons: Vec<String> = w
            .enabled_buttons()
            .into_iter()
            .map(|b| b.to_string())
            .collect();
        Ok(serde_json::to_string(&buttons).unwrap_or_default())
    })
}

#[napi]
fn window_set_minimized(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_minimized(value);
        Ok(())
    })
}

#[napi]
fn window_is_minimized(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_minimized()))
}

#[napi]
fn window_set_maximized(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_maximized(value);
        Ok(())
    })
}

#[napi]
fn window_is_maximized(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_maximized()))
}

#[napi]
fn window_fullscreen(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    crate::with_app_el(|app, el| {
        let fs = crate::application::fullscreen_from_value(app, el, &v)
            .unwrap_or(tao::window::Fullscreen::Borderless(None));
        with_window(&label, |w| {
            w.set_fullscreen(Some(fs));
            Ok(())
        })
    })
}

#[napi]
fn window_unfullscreen(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.set_fullscreen(None);
        Ok(())
    })
}

#[napi]
fn window_is_fullscreen(label: String) -> Result<String> {
    crate::with_app_el(|app, el| {
        with_window(&label, |w| {
            Ok(match w.fullscreen() {
                Some(tao::window::Fullscreen::Borderless(Some(monitor))) => app
                    .monitor_id(el, &monitor)
                    .map(|id| json!(id).to_string())
                    .unwrap_or_else(|| "true".to_string()),
                Some(_) => "true".to_string(),
                None => "false".to_string(),
            })
        })
    })
}

#[napi]
fn window_set_decorations(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_decorations(value);
        Ok(())
    })
}

#[napi]
fn window_is_decorated(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_decorated()))
}

#[napi]
fn window_set_always_on_top(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_always_on_top(value);
        Ok(())
    })
}

#[napi]
fn window_is_always_on_top(label: String) -> Result<bool> {
    with_window(&label, |w| Ok(w.is_always_on_top()))
}

#[napi]
fn window_set_always_on_bottom(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_always_on_bottom(value);
        Ok(())
    })
}

#[napi]
fn window_set_window_icon(label: String, path: String) -> Result<()> {
    with_window(&label, |w| {
        w.set_window_icon(&path)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_set_ime_position(label: String, data: String) -> Result<()> {
    let pos = position_from_value(&parse_json(&data))
        .ok_or_else(|| Error::from_reason("position is invalid"))?;
    with_window(&label, |w| {
        w.set_ime_position(pos);
        Ok(())
    })
}

#[napi]
fn window_set_progress_bar(label: String, data: String) -> Result<()> {
    let p = progress_value(&parse_json(&data)).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.set_progress_bar(p);
        Ok(())
    })
}

#[napi]
fn window_set_theme(label: String, data: String) -> Result<()> {
    let v = parse_json(&data);
    let theme = v.as_str().and_then(crate::application::theme_from_str);
    with_window(&label, |w| {
        w.set_theme(theme);
        Ok(())
    })
}

#[napi]
fn window_theme(label: String) -> Result<String> {
    with_window(&label, |w| {
        Ok(match w.theme() {
            tao::window::Theme::Light => "light".to_string(),
            tao::window::Theme::Dark => "dark".to_string(),
            _ => "light".to_string(),
        })
    })
}

#[napi]
fn window_set_content_protection(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_content_protection(value);
        Ok(())
    })
}

#[napi]
fn window_set_visible_on_all_workspaces(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_visible_on_all_workspaces(value);
        Ok(())
    })
}

#[napi]
fn window_id(label: String) -> Result<String> {
    with_window(&label, |w| Ok(w.id_string()))
}

// ===== 光标 =====

#[napi]
fn window_set_cursor_icon(label: String, cursor: String) -> Result<()> {
    let icon = cursor_icon_value(&cursor).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.set_cursor_icon(icon);
        Ok(())
    })
}

#[napi]
fn window_set_cursor_position(label: String, data: String) -> Result<()> {
    let pos = position_from_value(&parse_json(&data))
        .ok_or_else(|| Error::from_reason("position is invalid"))?;
    with_window(&label, |w| {
        w.set_cursor_position(pos)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_set_cursor_grab(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_cursor_grab(value)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_set_cursor_visible(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_cursor_visible(value);
        Ok(())
    })
}

#[napi]
fn window_drag_window(label: String) -> Result<()> {
    with_window(&label, |w| {
        w.drag_window()
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_drag_resize_window(label: String, direction: String) -> Result<()> {
    let dir = resize_direction_value(&direction).map_err(Error::from_reason)?;
    with_window(&label, |w| {
        w.drag_resize_window(dir)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_set_ignore_cursor_events(label: String, value: bool) -> Result<()> {
    with_window(&label, |w| {
        w.set_ignore_cursor_events(value)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

#[napi]
fn window_cursor_position(label: String) -> Result<String> {
    with_window(&label, |w| {
        let p = w
            .cursor_position()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(pos_to_json(p.x, p.y))
    })
}

// ===== 特殊操作（需要回调）=====

/// 执行 WebView JS 脚本并异步返回结果
#[napi]
fn evaluate_script(label: String, script: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    with_window(&label, |w| {
        w.evaluate_script_with_callback(&script, move |result| {
            tsfn.call(
                result,
                napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking,
            );
        })
        .map_err(|e| Error::from_reason(e.to_string()))
    })
}

/// Host→WebView RPC 请求（延迟响应）
#[napi]
fn rpc_invoke(label: String, method: String, data: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    let rpc_data: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
    crate::with_app(|app| {
        let win = app
            .get_window(&label)
            .ok_or_else(|| Error::from_reason(format!("window '{}' does not exist", label)))?;
        let rpc_id = win
            .rpc_state
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?
            .assign_host_request_id(move |result| {
                let json = match result {
                    Ok(data) => serde_json::to_string(&json!({"data": data})).unwrap_or_default(),
                    Err(error) => {
                        serde_json::to_string(&json!({"error": error})).unwrap_or_default()
                    }
                };
                tsfn.call(
                    json,
                    napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking,
                );
            });
        let payload = json!({ "id": rpc_id, "method": method, "data": rpc_data });
        let js = format!(
            "window.__taowry && window.__taowry._handleInvoke({})",
            serde_json::to_string(&payload).unwrap()
        );
        win.evaluate_script(&js)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

/// Host 回复 WebView→Host 的 RPC 请求
#[napi]
fn rpc_resolve(label: String, rpc_id: i64, data: String, error: Option<String>) -> Result<()> {
    let rpc_data: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
    crate::with_app(|app| {
        let win = app
            .get_window(&label)
            .ok_or_else(|| Error::from_reason(format!("window '{}' does not exist", label)))?;
        let data_json = serde_json::to_string(&rpc_data).unwrap();
        let error_json = match error {
            Some(e) => serde_json::to_string(&e).unwrap(),
            None => "null".to_string(),
        };
        let js = format!(
            "window.__taowry && window.__taowry._resolve({}, {}, {})",
            rpc_id, data_json, error_json
        );
        win.evaluate_script(&js)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

/// Host→WebView 单向 RPC 消息（fire-and-forget）
#[napi]
fn rpc_send(label: String, event: String, data: String) -> Result<()> {
    let rpc_data: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
    crate::with_app(|app| {
        let win = app
            .get_window(&label)
            .ok_or_else(|| Error::from_reason(format!("window '{}' does not exist", label)))?;
        let event_json = serde_json::to_string(&event).unwrap();
        let data_json = serde_json::to_string(&rpc_data).unwrap();
        let js = format!(
            "window.__taowry && window.__taowry._handleSend({}, {})",
            event_json, data_json
        );
        win.evaluate_script(&js)
            .map_err(|e| Error::from_reason(e.to_string()))
    })
}

/// 回复自定义协议请求（body 为 Buffer 直传，零拷贝）
#[napi]
fn protocol_response(
    label: String,
    request_id: String,
    status_code: u32,
    headers: String,
    body: Buffer,
) -> Result<()> {
    let headers_value: Value =
        serde_json::from_str(&headers).unwrap_or(Value::Object(serde_json::Map::new()));
    crate::with_app(|app| {
        let win = app
            .get_window(&label)
            .ok_or_else(|| Error::from_reason(format!("window '{}' does not exist", label)))?;
        let body: Vec<u8> = body.into();
        let mut response_builder = wry::http::Response::builder().status(status_code as u16);
        if let Some(headers_obj) = headers_value.as_object() {
            for (key, value) in headers_obj {
                if let Some(v) = value.as_str() {
                    response_builder = response_builder.header(key.as_str(), v);
                }
            }
        }
        let response = response_builder
            .body(body)
            .map_err(|e| Error::from_reason(format!("failed to build response: {}", e)))?;
        let responder = win
            .protocol_state
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?
            .remove(&request_id);
        match responder {
            Some(responder) => {
                responder.respond(response);
                Ok(())
            }
            None => Err(Error::from_reason(format!(
                "protocol request '{}' not found",
                request_id
            ))),
        }
    })
}
