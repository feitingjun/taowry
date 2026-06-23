//! Utils napi 导出函数 — Node 端直接调用

use napi::bindgen_prelude::*;
use serde_json::Value;

use super::helpers::parse_json;

// ===== Fire-and-forget =====

#[napi]
fn utils_notify(data: String) -> Result<()> {
    let v = parse_json(&data);
    let title = v.get("title").and_then(Value::as_str).unwrap_or("");
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    crate::utils::send_notification(title, None, body);
    Ok(())
}

#[napi]
fn utils_open_file(path: String) -> Result<()> {
    let safe = crate::utils::sanitize_open_path(&path)
        .ok_or_else(|| Error::from_reason("unsafe file path"))?;
    open::that_detached(&safe).map_err(|e| Error::from_reason(e.to_string()))
}

#[napi]
fn utils_open_url(url: String) -> Result<()> {
    if !crate::utils::is_safe_open_url(&url) {
        return Err(Error::from_reason("unsafe URL protocol"));
    }
    open::that(&url).map_err(|e| Error::from_reason(e.to_string()))
}

// ===== 异步文件对话框（后台线程 + tsfn 回调）=====

/// 从 JSON 解析过滤器列表
fn parse_filters(data: &Value) -> Vec<(String, Vec<String>)> {
    data.get("filters")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let fo = f.as_object()?;
                    let name = fo.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                    let exts: Vec<String> = fo
                        .get("extensions")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default();
                    Some((name, exts))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 配置同步文件对话框

#[napi]
fn utils_pick_file(data: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    let v = parse_json(&data);
    let filters = parse_filters(&v);
    let directory = v.get("directory").and_then(Value::as_str).map(String::from);
    let file_name = v.get("fileName").and_then(Value::as_str).map(String::from);
    let cmd = crate::utils::UtilCommand::PickFile { id: 0, filters, directory, file_name };
    crate::with_app(|app| {
        app.pending_node_dialogs.push((cmd, tsfn));
        Ok(())
    })
}

#[napi]
fn utils_pick_files(data: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    let v = parse_json(&data);
    let filters = parse_filters(&v);
    let directory = v.get("directory").and_then(Value::as_str).map(String::from);
    let cmd = crate::utils::UtilCommand::PickFiles { id: 0, filters, directory };
    crate::with_app(|app| {
        app.pending_node_dialogs.push((cmd, tsfn));
        Ok(())
    })
}

#[napi]
fn utils_pick_folder(data: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    let v = parse_json(&data);
    let directory = v.get("directory").and_then(Value::as_str).map(String::from);
    let cmd = crate::utils::UtilCommand::PickFolder { id: 0, directory };
    crate::with_app(|app| {
        app.pending_node_dialogs.push((cmd, tsfn));
        Ok(())
    })
}

#[napi]
fn utils_save_file(data: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    let v = parse_json(&data);
    let filters = parse_filters(&v);
    let directory = v.get("directory").and_then(Value::as_str).map(String::from);
    let file_name = v.get("fileName").and_then(Value::as_str).map(String::from);
    let cmd = crate::utils::UtilCommand::SaveFile { id: 0, filters, directory, file_name };
    crate::with_app(|app| {
        app.pending_node_dialogs.push((cmd, tsfn));
        Ok(())
    })
}

// ===== 消息对话框 =====

#[napi]
fn utils_show_message(data: String, callback: JsFunction) -> Result<()> {
    let tsfn: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    > = callback.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<String>| {
            ctx.env.create_string_from_std(ctx.value).map(|v| vec![v])
        },
    )?;
    let v = parse_json(&data);
    std::thread::spawn(move || {
        let title = v.get("title").and_then(Value::as_str).unwrap_or("");
        let body = v.get("body").and_then(Value::as_str).unwrap_or("");
        let level = match v.get("level").and_then(Value::as_str) {
            Some("warning") => crate::utils::MessageLevel::Warning,
            Some("error") => crate::utils::MessageLevel::Error,
            _ => crate::utils::MessageLevel::Info,
        };
        let buttons = match v.get("buttons") {
            Some(Value::String(s)) => match s.as_str() {
                "okCancel" => crate::utils::ButtonsConfig::OkCancel,
                "yesNo" => crate::utils::ButtonsConfig::YesNo,
                "yesNoCancel" => crate::utils::ButtonsConfig::YesNoCancel,
                _ => crate::utils::ButtonsConfig::Ok,
            },
            Some(Value::Array(arr)) => {
                let labels: Vec<String> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect();
                if labels.is_empty() {
                    crate::utils::ButtonsConfig::Ok
                } else {
                    crate::utils::ButtonsConfig::Custom(labels)
                }
            }
            _ => crate::utils::ButtonsConfig::Ok,
        };
        let result = crate::utils::show_sync_message_dialog(title, body, &level, &buttons);
        let value = crate::utils::dialog_result_to_value(&result);
        let json_str = value.to_string();
        tsfn.call(json_str, napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking);
    });
    Ok(())
}

// ===== 目录查询 =====

#[napi]
fn utils_get_dir(name: String) -> Result<String> {
    let dir_type = match name.as_str() {
        "desktop" => crate::utils::DirType::Desktop,
        "documents" => crate::utils::DirType::Documents,
        "downloads" => crate::utils::DirType::Downloads,
        "pictures" => crate::utils::DirType::Pictures,
        "music" => crate::utils::DirType::Music,
        "videos" => crate::utils::DirType::Videos,
        "home" => crate::utils::DirType::Home,
        "temp" => crate::utils::DirType::Temp,
        "appData" => crate::utils::DirType::AppData,
        "appConfig" => crate::utils::DirType::AppConfig,
        "appCache" => crate::utils::DirType::AppCache,
        "appLog" => crate::utils::DirType::AppLog,
        _ => return Err(Error::from_reason(format!("unknown directory type: {}", name))),
    };
    // 对于应用目录，检查 app_name 是否已设置
    let is_app_dir = matches!(
        dir_type,
        crate::utils::DirType::AppData
            | crate::utils::DirType::AppConfig
            | crate::utils::DirType::AppCache
            | crate::utils::DirType::AppLog
    );
    if is_app_dir && crate::utils::get_app_name().is_none() {
        return Err(Error::from_reason(
            "Application appName not set. Pass { appName: '...' } to new Application() before calling app-scoped directory APIs.",
        ));
    }
    match crate::utils::get_dir_path(&dir_type) {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Ok("null".to_string()),
    }
}
