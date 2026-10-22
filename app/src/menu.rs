//! 托盘菜单构建与剪贴板操作模块。

use std::collections::HashMap;
use std::ptr;
use muda::{ContextMenu, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::UI::WindowsAndMessaging::{CheckMenuItem, MF_BYPOSITION, MF_CHECKED};

use crate::logic::{get_all_info_text, PhoneInfo, STATE_TEXT_OFF, STATE_TEXT_ON};

/// Windows 剪贴板标准 Unicode 文本格式常量（CF_UNICODETEXT = 13）。
pub const CF_UNICODETEXT: u32 = 13;

/// 菜单项点击后所触发的业务动作定义。
#[derive(Debug, Clone)]
pub enum MenuAction {
    /// 切换某台手机的网络共享状态（当前共享则停止，未共享则启用）。
    ToggleSharing {
        /// 手机序列号。
        serial: String,
        /// 手机显示名称。
        name: String,
        /// 当前是否正处于共享状态。
        current_state: bool,
    },
    /// 复制某单个属性的值到剪贴板。
    CopyText {
        /// 属性显示名称（如 "型号"）。
        label: String,
        /// 待复制的属性具体文本值。
        value: String,
    },
    /// 复制一台手机的全部详细信息。
    CopyAllInfo {
        /// 手机序列号。
        serial: String,
        /// 拼接好的全部信息文本。
        text: String,
    },
    /// 对所有已连接的手机启用网络共享。
    EnableAll,
    /// 对所有已连接的手机停止网络共享。
    DisableAll,
    /// 退出托盘程序。
    Quit,
}

/// 将指定的 Unicode 文本安全写入 Windows 系统剪贴板（使用 CF_UNICODETEXT 格式）。
///
/// # 参数
/// - `text`: 待写入剪贴板的字符串。
///
/// # 返回值
/// 写入成功返回 `Ok(())`，调用 Windows API 失败返回错误描述。
///
/// # 示例
/// ```no_run
/// use phone_netshare::menu::copy_text_to_clipboard;
/// copy_text_to_clipboard("STG_AL00").expect("写入剪贴板失败");
/// ```
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if hmem.is_null() {
            return Err("GlobalAlloc 分配剪贴板内存失败".to_string());
        }

        let locked = GlobalLock(hmem);
        if locked.is_null() {
            return Err("GlobalLock 锁定剪贴板内存失败".to_string());
        }
        ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, locked as *mut u8, bytes_len);
        GlobalUnlock(hmem);

        // 尝试打开剪贴板，带有多次简短重试
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if !opened {
            return Err("OpenClipboard 打开系统剪贴板失败".to_string());
        }

        EmptyClipboard();
        let res = SetClipboardData(CF_UNICODETEXT, hmem);
        CloseClipboard();

        if res.is_null() {
            return Err("SetClipboardData 写入剪贴板失败".to_string());
        }
    }

    Ok(())
}

/// 某台手机在菜单中呈现的聚合状态数据。
#[derive(Debug, Clone)]
pub struct PhoneMenuEntry {
    /// 手机序列号。
    pub serial: String,
    /// 手机标签名称（已处理同型号重名尾缀）。
    pub label: String,
    /// 当前是否处于共享状态。
    pub sharing: bool,
    /// 手机的详细属性（若后台尚未读完可为空）。
    pub info: Option<PhoneInfo>,
}

/// 构建托盘右键/左键主菜单，并返回菜单对象及其菜单项 ID 到业务动作的映射表。
///
/// # 参数
/// - `adb_found`: 当前 PATH 中是否成功检索到了 adb。
/// - `entries`: 准备展示的手机状态列表。
///
/// # 返回值
/// 包含构建完成的 [`muda::Menu`] 以及 [`HashMap<MenuId, MenuAction>`] 的二元组。
///
/// # 示例
/// ```
/// use phone_netshare::menu::build_tray_menu;
/// let (menu, action_map) = build_tray_menu(true, &[]);
/// ```
pub fn build_tray_menu(
    adb_found: bool,
    entries: &[PhoneMenuEntry],
) -> (Menu, HashMap<MenuId, MenuAction>) {
    let menu = Menu::new();
    let mut map = HashMap::new();

    if !adb_found {
        let warn_item = MenuItem::new("找不到 adb，请安装 Android platform-tools 并加入 PATH", false, None);
        let _ = menu.append(&warn_item);
    } else if entries.is_empty() {
        let empty_item = MenuItem::new("没有检测到手机", false, None);
        let _ = menu.append(&empty_item);
    } else {
        for entry in entries {
            // 主菜单项只放手机名，共享中的选中符号稍后用系统的 CheckMenuItem 标在左侧
            let submenu = Submenu::new(&entry.label, true);

            // 1. 首项：文字随状态在「启用共享」「停用共享」间切换，点一下切换
            let toggle_title = if entry.sharing { "停用共享" } else { "启用共享" };
            let toggle_item = MenuItem::new(toggle_title, true, None);
            map.insert(
                toggle_item.id().clone(),
                MenuAction::ToggleSharing {
                    serial: entry.serial.clone(),
                    name: entry.label.clone(),
                    current_state: entry.sharing,
                },
            );
            let _ = submenu.append(&toggle_item);

            let _ = submenu.append(&PredefinedMenuItem::separator());

            // 2. 每行 项目：值 (设备名、型号、品牌、制造商、设备代号、系统、CPU 架构、序列号)
            let detail_rows: Vec<(&str, String)> = if let Some(info) = &entry.info {
                vec![
                    ("设备名", info.name.clone()),
                    ("型号", info.model.clone()),
                    ("品牌", info.brand.clone()),
                    ("制造商", info.manufacturer.clone()),
                    ("设备代号", info.device.clone()),
                    ("系统", info.os.clone()),
                    ("CPU 架构", info.abi.clone()),
                    ("序列号", info.serial.clone()),
                ]
            } else {
                vec![("序列号", entry.serial.clone())]
            };

            for (label, val) in detail_rows {
                let row_text = format!("{label}：{val}");
                let row_item = MenuItem::new(&row_text, true, None);
                map.insert(
                    row_item.id().clone(),
                    MenuAction::CopyText {
                        label: label.to_string(),
                        value: val,
                    },
                );
                let _ = submenu.append(&row_item);
            }

            // 3. 底部：分隔线 + 复制全部信息
            let _ = submenu.append(&PredefinedMenuItem::separator());

            let copy_all_item = MenuItem::new("复制全部信息", true, None);
            let full_text = match &entry.info {
                Some(info) => get_all_info_text(entry.sharing, info),
                None => format!(
                    "连接状态：{}\r\n序列号：{}",
                    if entry.sharing {
                        STATE_TEXT_ON
                    } else {
                        STATE_TEXT_OFF
                    },
                    entry.serial
                ),
            };
            map.insert(
                copy_all_item.id().clone(),
                MenuAction::CopyAllInfo {
                    serial: entry.serial.clone(),
                    text: full_text,
                },
            );
            let _ = submenu.append(&copy_all_item);

            let _ = menu.append(&submenu);
        }

        // 共享中的手机：用系统自带的选中符号标在主菜单项左侧（按位置，第 i 项对应第 i 台手机）
        let hmenu = menu.hpopupmenu() as *mut core::ffi::c_void;
        for (i, entry) in entries.iter().enumerate() {
            if entry.sharing {
                unsafe {
                    CheckMenuItem(hmenu, i as u32, MF_BYPOSITION | MF_CHECKED);
                }
            }
        }
    }

    // 底部通用菜单项
    let _ = menu.append(&PredefinedMenuItem::separator());

    let enable_all_item = MenuItem::new("全部启用", !entries.is_empty(), None);
    map.insert(enable_all_item.id().clone(), MenuAction::EnableAll);
    let _ = menu.append(&enable_all_item);

    let disable_all_item = MenuItem::new("全部停止", true, None);
    map.insert(disable_all_item.id().clone(), MenuAction::DisableAll);
    let _ = menu.append(&disable_all_item);

    let _ = menu.append(&PredefinedMenuItem::separator());

    let quit_item = MenuItem::new("退出", true, None);
    map.insert(quit_item.id().clone(), MenuAction::Quit);
    let _ = menu.append(&quit_item);

    (menu, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_build_empty() {
        let (menu, map) = build_tray_menu(true, &[]);
        assert!(!map.is_empty(), "必须包含退出与全部操作项");
        drop(menu);
    }
}
