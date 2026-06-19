//! 菜单构建模块
//!
//! 提供菜单类型定义和菜单项构建相关的辅助函数。
//! Application 结构体仍然负责菜单的生命周期管理。

use serde_json::Value;
use tray_icon::menu::{
    accelerator::Accelerator, CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu,
};

/// 被管理的菜单类型（顶层菜单或子菜单）
#[derive(Clone)]
pub enum ManagedMenu {
    Menu(Menu),
    Submenu(Submenu),
}

/// 被管理的菜单项类型
#[derive(Clone)]
pub enum ManagedMenuItem {
    Normal(MenuItem),
    Check(CheckMenuItem),
    Predefined(PredefinedMenuItem),
    Submenu(Submenu),
}

impl ManagedMenu {
    pub fn as_context_menu(&self) -> Result<Box<dyn tray_icon::menu::ContextMenu>, String> {
        match self {
            ManagedMenu::Menu(menu) => Ok(Box::new(menu.clone())),
            ManagedMenu::Submenu(submenu) => Ok(Box::new(submenu.clone())),
        }
    }

    pub fn as_root_menu(&self) -> Result<Menu, String> {
        match self {
            ManagedMenu::Menu(menu) => Ok(menu.clone()),
            ManagedMenu::Submenu(_) => {
                Err("a submenu cannot be used as an application/window menu".to_string())
            }
        }
    }
}

/// 将菜单项添加到菜单或子菜单中
pub fn append_item_to_menu(menu: &ManagedMenu, item: &ManagedMenuItem) -> Result<(), String> {
    match (menu, item) {
        (ManagedMenu::Menu(menu), ManagedMenuItem::Normal(item)) => append(menu, item),
        (ManagedMenu::Menu(menu), ManagedMenuItem::Check(item)) => append(menu, item),
        (ManagedMenu::Menu(menu), ManagedMenuItem::Predefined(item)) => append(menu, item),
        (ManagedMenu::Menu(menu), ManagedMenuItem::Submenu(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Normal(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Check(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Predefined(item)) => append(menu, item),
        (ManagedMenu::Submenu(menu), ManagedMenuItem::Submenu(item)) => append(menu, item),
    }
}

fn append(menu: &impl MenuAppender, item: &dyn IsMenuItem) -> Result<(), String> {
    menu.append_item(item)
}

trait MenuAppender {
    fn append_item(&self, item: &dyn IsMenuItem) -> Result<(), String>;
}

impl MenuAppender for Menu {
    fn append_item(&self, item: &dyn IsMenuItem) -> Result<(), String> {
        self.append(item)
            .map_err(|error| format!("failed to append menu item: {}", error))
    }
}

impl MenuAppender for Submenu {
    fn append_item(&self, item: &dyn IsMenuItem) -> Result<(), String> {
        self.append(item)
            .map_err(|error| format!("failed to append submenu item: {}", error))
    }
}

/// 获取被管理菜单项的 id
pub fn managed_item_id(item: &ManagedMenuItem) -> String {
    match item {
        ManagedMenuItem::Normal(item) => item.id().as_ref().to_string(),
        ManagedMenuItem::Check(item) => item.id().as_ref().to_string(),
        ManagedMenuItem::Predefined(item) => item.id().as_ref().to_string(),
        ManagedMenuItem::Submenu(item) => item.id().as_ref().to_string(),
    }
}

/// 从 JSON 解析快捷键
pub fn parse_accelerator(data: &Value) -> Result<Option<Accelerator>, String> {
    match data.get("accelerator").and_then(Value::as_str) {
        Some(accelerator) => accelerator
            .parse()
            .map(Some)
            .map_err(|error| format!("invalid accelerator '{}': {}", accelerator, error)),
        None => Ok(None),
    }
}

/// 构建预定义菜单项
pub fn build_predefined_item(kind: &str, text: Option<&str>) -> Result<PredefinedMenuItem, String> {
    Ok(match kind {
        "copy" => PredefinedMenuItem::copy(text),
        "cut" => PredefinedMenuItem::cut(text),
        "paste" => PredefinedMenuItem::paste(text),
        "selectAll" => PredefinedMenuItem::select_all(text),
        "undo" => PredefinedMenuItem::undo(text),
        "redo" => PredefinedMenuItem::redo(text),
        "minimize" => PredefinedMenuItem::minimize(text),
        "maximize" => PredefinedMenuItem::maximize(text),
        "fullscreen" => PredefinedMenuItem::fullscreen(text),
        "hide" => PredefinedMenuItem::hide(text),
        "hideOthers" => PredefinedMenuItem::hide_others(text),
        "showAll" => PredefinedMenuItem::show_all(text),
        "closeWindow" => PredefinedMenuItem::close_window(text),
        "quit" => PredefinedMenuItem::quit(text),
        "services" => PredefinedMenuItem::services(text),
        "bringAllToFront" => PredefinedMenuItem::bring_all_to_front(text),
        "separator" => PredefinedMenuItem::separator(),
        other => return Err(format!("unsupported predefined menu item '{}'", other)),
    })
}
