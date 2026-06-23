//! Utils 工具模块
//!
//! 提供文件对话框、系统通知、消息弹窗、打开文件/URL、OS 目录等实用功能。
//! 支持通过命令队列从 WebView 直通调用（client 端），以及通过 napi 直接调用（Node 端）。

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, LazyLock};

// ===== 应用名称（用户自定义，可选） =====

static APP_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// 设置应用名称（由 Application 构造函数调用）
pub fn set_app_name(name: &str) {
    if let Ok(mut app_name) = APP_NAME.lock() {
        *app_name = name.to_string();
    }
}

/// 获取应用名称，未设置时返回 None
pub fn get_app_name() -> Option<String> {
    APP_NAME.lock().ok().and_then(|name| {
        if name.is_empty() {
            None
        } else {
            Some(name.clone())
        }
    })
}

// ===== 类型定义 =====

/// 工具命令队列类型
pub type UtilCommandQueue = Arc<Mutex<Vec<UtilCommand>>>;

/// 工具命令枚举
pub enum UtilCommand {
    // ── Fire-and-forget ──
    Notify {
        title: String,
        subtitle: Option<String>,
        body: String,
    },
    OpenFile(String),
    OpenUrl(String),

    // ── 同步查询（立即 resolve）──
    GetDir { id: u64, dir_type: DirType },

    // ── 异步对话框（创建 Future 加入 pending 队列）──
    PickFile {
        id: u64,
        filters: Vec<(String, Vec<String>)>,
        directory: Option<String>,
        file_name: Option<String>,
    },
    PickFiles {
        id: u64,
        filters: Vec<(String, Vec<String>)>,
        directory: Option<String>,
    },
    PickFolder {
        id: u64,
        directory: Option<String>,
    },
    SaveFile {
        id: u64,
        filters: Vec<(String, Vec<String>)>,
        directory: Option<String>,
        file_name: Option<String>,
    },
    ShowMessage {
        id: u64,
        title: String,
        body: String,
        detail: Option<String>,
        level: MessageLevel,
        buttons: ButtonsConfig,
    },
}

/// 目录类型
pub enum DirType {
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Home,
    Temp,
    AppData,
    AppConfig,
    AppCache,
    AppLog,
}

/// 消息对话框级别
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

/// 消息对话框按钮配置
pub enum ButtonsConfig {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    Custom(Vec<String>),
}

/// 异步工具操作的结果
pub enum UtilResult {
    Json(Value),
    Error(String),
}

// ===== 解析函数 =====


// ===== 执行函数 =====

/// 发送系统通知
///
/// macOS 上使用 `osascript` 的 `display notification` 命令（原生通知，无需选择应用），
/// 其他平台使用 `notify-rust`。
pub fn send_notification(title: &str, subtitle: Option<&str>, body: &str) {
    #[cfg(target_os = "macos")]
    {
        let mut script = format!(
            "display notification \"{}\"",
            escape_applescript(body),
        );
        if let Some(sub) = subtitle {
            if !sub.is_empty() {
                script.push_str(&format!(" subtitle \"{}\"", escape_applescript(sub)));
            }
        }
        script.push_str(&format!(" with title \"{}\"", escape_applescript(title)));
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut n = notify_rust::Notification::new();
        n.summary(title);
        if let Some(sub) = subtitle {
            if !sub.is_empty() {
                n.body(&format!("{}\n{}", sub, body));
            } else {
                n.body(body);
            }
        } else {
            n.body(body);
        }
        let _ = n.show();
    }
}

/// 转义 AppleScript 字符串中的特殊字符
#[cfg(target_os = "macos")]
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 执行 fire-and-forget 命令
pub fn execute_fire_and_forget(cmd: &UtilCommand) {
    match cmd {
        UtilCommand::Notify { title, subtitle, body } => {
            send_notification(title, subtitle.as_deref(), body);
        }
        UtilCommand::OpenFile(path) => {
            if let Some(safe) = sanitize_open_path(path) {
                let _ = open::that_detached(&safe);
            }
        }
        UtilCommand::OpenUrl(url) => {
            if is_safe_open_url(url) {
                let _ = open::that(url);
            }
        }
        _ => {}
    }
}

/// 净化 openFile 路径，防止 ../ 越权和访问系统敏感目录
pub fn sanitize_open_path(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    // 拒绝以 / 开头的绝对系统路径（/bin, /System, /etc 等）
    // 仅允许相对路径或 ~ 开头的用户路径
    if raw.starts_with('/') {
        return None;
    }
    // 标准化路径并检查是否越权
    let path = std::path::Path::new(raw);
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                // 如果有 .. 组件，确保不越出当前目录
                if !normalized.pop() {
                    return None; // 根目录外
                }
            }
            Component::Normal(c) => normalized.push(c),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    if normalized.as_os_str().is_empty() {
        return None;
    }
    Some(normalized.to_string_lossy().to_string())
}

/// 检查 openUrl 是否安全（复用 SetUrl 的白名单逻辑 + 额外限制）
pub fn is_safe_open_url(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    // 仅允许 http/https，拒绝 file/javascript/data 等
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// 获取目录路径
///
/// 对于 AppData/AppConfig/AppCache/AppLog 类型，需要用户通过 `set_app_name` 设置应用名称，
/// 否则返回 None。
pub fn get_dir_path(dir_type: &DirType) -> Option<PathBuf> {
    match dir_type {
        DirType::Desktop => dirs::desktop_dir(),
        DirType::Documents => dirs::document_dir(),
        DirType::Downloads => dirs::download_dir(),
        DirType::Pictures => dirs::picture_dir(),
        DirType::Music => dirs::audio_dir(),
        DirType::Videos => dirs::video_dir(),
        DirType::Home => dirs::home_dir(),
        DirType::Temp => Some(std::env::temp_dir()),
        DirType::AppData => get_app_name().and_then(|name| dirs::data_dir().map(|p| p.join(&name))),
        DirType::AppConfig => get_app_name().and_then(|name| dirs::config_dir().map(|p| p.join(&name))),
        DirType::AppCache => get_app_name().and_then(|name| dirs::cache_dir().map(|p| p.join(&name))),
        DirType::AppLog => get_app_name().and_then(|name| dirs::data_dir().map(|p| p.join(&name).join("logs"))),
    }
}

// ===== 对话框（主线程同步执行）=====

/// 为同步 rfd::FileDialog 应用过滤器和目录配置
fn configure_dialog(
    mut dialog: rfd::FileDialog,
    filters: &[(String, Vec<String>)],
    directory: &Option<String>,
    file_name: &Option<String>,
) -> rfd::FileDialog {
    for (name, exts) in filters {
        dialog = dialog.add_filter(name, exts);
    }
    if let Some(dir) = directory {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = file_name {
        dialog = dialog.set_file_name(name);
    }
    dialog
}

/// 在主线程同步执行对话框命令，结果直接通过 evaluate_script 回传 WebView。
/// 必须在主线程（事件泵回调）中调用。
pub fn execute_dialog_sync(cmd: &UtilCommand, window: &crate::window::BrowserWindow) {
    // macOS: 对话框需要 App 处于前台激活状态，否则面板一闪即关闭
    #[cfg(target_os = "macos")]
    let _guard = MacOSAppActivator::new();

    match cmd {
        UtilCommand::PickFile { id, filters, directory, file_name } => {
            let dialog = configure_dialog(rfd::FileDialog::new(), filters, directory, file_name);
            let result = dialog.pick_file();
            let value = match result {
                Some(path) => serde_json::json!(path.to_string_lossy().to_string()),
                None => serde_json::Value::Null,
            };
            let data = serde_json::to_string(&value).unwrap_or_default();
            let js = format!(
                "window.__taowry && window.__taowry._resolve({}, {}, null)",
                id, data
            );
            let _ = window.evaluate_script(&js);
        }
        UtilCommand::PickFiles { id, filters, directory } => {
            let dialog = configure_dialog(rfd::FileDialog::new(), filters, directory, &None);
            let result = dialog.pick_files();
            let value = match result {
                Some(paths) => {
                    let paths: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
                    serde_json::json!(paths)
                }
                None => serde_json::json!([]),
            };
            let data = serde_json::to_string(&value).unwrap_or_default();
            let js = format!("window.__taowry && window.__taowry._resolve({}, {}, null)", id, data);
            let _ = window.evaluate_script(&js);
        }
        UtilCommand::PickFolder { id, directory } => {
            let mut dialog = rfd::FileDialog::new();
            if let Some(dir) = directory {
                dialog = dialog.set_directory(dir);
            }
            let value = match dialog.pick_folder() {
                Some(path) => serde_json::json!(path.to_string_lossy().to_string()),
                None => serde_json::Value::Null,
            };
            let data = serde_json::to_string(&value).unwrap_or_default();
            let js = format!("window.__taowry && window.__taowry._resolve({}, {}, null)", id, data);
            let _ = window.evaluate_script(&js);
        }
        UtilCommand::SaveFile { id, filters, directory, file_name } => {
            let dialog = configure_dialog(rfd::FileDialog::new(), filters, directory, file_name);
            let value = match dialog.save_file() {
                Some(path) => serde_json::json!(path.to_string_lossy().to_string()),
                None => serde_json::Value::Null,
            };
            let data = serde_json::to_string(&value).unwrap_or_default();
            let js = format!("window.__taowry && window.__taowry._resolve({}, {}, null)", id, data);
            let _ = window.evaluate_script(&js);
        }
        UtilCommand::ShowMessage { id, title, body, detail, level, buttons } => {
            let rfd_level = match level {
                MessageLevel::Info => rfd::MessageLevel::Info,
                MessageLevel::Warning => rfd::MessageLevel::Warning,
                MessageLevel::Error => rfd::MessageLevel::Error,
            };
            let rfd_buttons = match buttons {
                ButtonsConfig::Ok => rfd::MessageButtons::Ok,
                ButtonsConfig::OkCancel => rfd::MessageButtons::OkCancel,
                ButtonsConfig::YesNo => rfd::MessageButtons::YesNo,
                ButtonsConfig::YesNoCancel => rfd::MessageButtons::YesNoCancel,
                ButtonsConfig::Custom(labels) => {
                    if labels.len() >= 3 {
                        rfd::MessageButtons::YesNoCancelCustom(labels[0].clone(), labels[1].clone(), labels[2].clone())
                    } else if labels.len() == 2 {
                        rfd::MessageButtons::OkCancelCustom(labels[0].clone(), labels[1].clone())
                    } else {
                        rfd::MessageButtons::Ok
                    }
                }
            };
            let description = match detail {
                Some(d) if !d.is_empty() => format!("{}\n\n{}", body, d),
                _ => body.clone(),
            };
            let result = rfd::MessageDialog::new()
                .set_title(title)
                .set_description(&description)
                .set_level(rfd_level)
                .set_buttons(rfd_buttons)
                .show();
            let index = match &result {
                rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes => 0,
                rfd::MessageDialogResult::Cancel => 1,
                rfd::MessageDialogResult::No => 0,
                rfd::MessageDialogResult::Custom(_) => 0,
            };
            let js = format!("window.__taowry && window.__taowry._resolve({}, {}, null)", id, index);
            let _ = window.evaluate_script(&js);
        }
        _ => {}
    }
}

/// 运行对话框并返回 JSON 值（供 Node 端和 WebView 端共享使用）
pub fn run_dialog_to_value(cmd: &UtilCommand) -> Value {
    #[cfg(target_os = "macos")]
    let _guard = MacOSAppActivator::new();

    match cmd {
        UtilCommand::PickFile { filters, directory, file_name, .. } => {
            configure_dialog(rfd::FileDialog::new(), filters, directory, file_name)
                .pick_file()
                .map(|p| json!(p.to_string_lossy().to_string()))
                .unwrap_or(Value::Null)
        }
        UtilCommand::PickFiles { filters, directory, .. } => {
            configure_dialog(rfd::FileDialog::new(), filters, directory, &None)
                .pick_files()
                .map(|paths| {
                    json!(paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>())
                })
                .unwrap_or_else(|| json!([]))
        }
        UtilCommand::PickFolder { directory, .. } => {
            let mut d = rfd::FileDialog::new();
            if let Some(dir) = directory { d = d.set_directory(dir); }
            d.pick_folder()
                .map(|p| json!(p.to_string_lossy().to_string()))
                .unwrap_or(Value::Null)
        }
        UtilCommand::SaveFile { filters, directory, file_name, .. } => {
            configure_dialog(rfd::FileDialog::new(), filters, directory, file_name)
                .save_file()
                .map(|p| json!(p.to_string_lossy().to_string()))
                .unwrap_or(Value::Null)
        }
        UtilCommand::ShowMessage { title, body, detail, level, buttons, .. } => {
            let rfd_level = match level {
                MessageLevel::Info => rfd::MessageLevel::Info,
                MessageLevel::Warning => rfd::MessageLevel::Warning,
                MessageLevel::Error => rfd::MessageLevel::Error,
            };
            let rfd_buttons = match buttons {
                ButtonsConfig::Ok => rfd::MessageButtons::Ok,
                ButtonsConfig::OkCancel => rfd::MessageButtons::OkCancel,
                ButtonsConfig::YesNo => rfd::MessageButtons::YesNo,
                ButtonsConfig::YesNoCancel => rfd::MessageButtons::YesNoCancel,
                ButtonsConfig::Custom(labels) => {
                    if labels.len() >= 3 {
                        rfd::MessageButtons::YesNoCancelCustom(labels[0].clone(), labels[1].clone(), labels[2].clone())
                    } else if labels.len() == 2 {
                        rfd::MessageButtons::OkCancelCustom(labels[0].clone(), labels[1].clone())
                    } else {
                        rfd::MessageButtons::Ok
                    }
                }
            };
            let description = match detail {
                Some(d) if !d.is_empty() => format!("{}\n\n{}", body, d),
                _ => body.clone(),
            };
            let result = rfd::MessageDialog::new()
                .set_title(title)
                .set_description(&description)
                .set_level(rfd_level)
                .set_buttons(rfd_buttons)
                .show();
            let index = match &result {
                rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes => 0,
                rfd::MessageDialogResult::Cancel => 1,
                rfd::MessageDialogResult::No => 0,
                rfd::MessageDialogResult::Custom(_) => 0,
            };
            json!(index)
        }
        _ => Value::Null,
    }
}

// ===== macOS 对话框激活辅助 =====

#[cfg(target_os = "macos")]
struct MacOSAppActivator {
    prev_policy: i64,
}

#[cfg(target_os = "macos")]
impl MacOSAppActivator {
    fn new() -> Self {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;

        unsafe {
            let app_cls = objc2::class!(NSApplication);
            let app: *mut AnyObject = msg_send![app_cls, sharedApplication];

            // 保存当前激活策略
            let prev_policy: i64 = msg_send![app, activationPolicy];

            // 设置为 Regular 前台应用策略（对话框需要）
            // NSApplicationActivationPolicyRegular = 0
            let _: () = msg_send![app, setActivationPolicy: 0i64];

            // 激活应用，使其成为前台应用
            let _: () = msg_send![app, activateIgnoringOtherApps: true];

            Self { prev_policy }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacOSAppActivator {
    fn drop(&mut self) {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;

        unsafe {
            let app_cls = objc2::class!(NSApplication);
            let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
            // 恢复原始激活策略
            let _: () = msg_send![app, setActivationPolicy: self.prev_policy];
        }
    }
}

// ===== Node 端辅助函数（供 napi 使用）=====

/// 配置同步文件对话框
pub fn configure_sync_file_dialog(
    mut dialog: rfd::FileDialog,
    filters: &[(String, Vec<String>)],
    directory: &Option<String>,
    file_name: &Option<String>,
) -> rfd::FileDialog {
    for (name, exts) in filters {
        dialog = dialog.add_filter(name, exts);
    }
    if let Some(dir) = directory {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = file_name {
        dialog = dialog.set_file_name(name);
    }
    dialog
}

/// 构建同步消息对话框并显示
pub fn show_sync_message_dialog(
    title: &str,
    body: &str,
    level: &MessageLevel,
    buttons: &ButtonsConfig,
) -> rfd::MessageDialogResult {
    let rfd_level = match level {
        MessageLevel::Info => rfd::MessageLevel::Info,
        MessageLevel::Warning => rfd::MessageLevel::Warning,
        MessageLevel::Error => rfd::MessageLevel::Error,
    };
    let rfd_buttons = match buttons {
        ButtonsConfig::Ok => rfd::MessageButtons::Ok,
        ButtonsConfig::OkCancel => rfd::MessageButtons::OkCancel,
        ButtonsConfig::YesNo => rfd::MessageButtons::YesNo,
        ButtonsConfig::YesNoCancel => rfd::MessageButtons::YesNoCancel,
        ButtonsConfig::Custom(labels) => {
            if labels.len() >= 3 {
                rfd::MessageButtons::YesNoCancelCustom(
                    labels[0].clone(),
                    labels[1].clone(),
                    labels[2].clone(),
                )
            } else if labels.len() == 2 {
                rfd::MessageButtons::OkCancelCustom(labels[0].clone(), labels[1].clone())
            } else {
                rfd::MessageButtons::Ok
            }
        }
    };
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(body)
        .set_level(rfd_level)
        .set_buttons(rfd_buttons)
        .show()
}

/// 将 MessageDialogResult 转换为 JSON 值
pub fn dialog_result_to_value(result: &rfd::MessageDialogResult) -> Value {
    match result {
        rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes => json!(true),
        rfd::MessageDialogResult::No | rfd::MessageDialogResult::Cancel => json!(false),
        rfd::MessageDialogResult::Custom(label) => json!(label),
    }
}
