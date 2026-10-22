//! 托盘图标：直接嵌入原 PowerShell 版（NetShareIcon.psm1）画好的像素，保证和原版一模一样。
//! 像素由 `scripts/export-icons.ps1` 导出，要改图标就改 psm1 再重新导出。

use tray_icon::Icon;

/// 图标边长（像素），与原版托盘图标一致。
pub const ICON_SIZE: u32 = 32;

/// 空闲（灰色手机）图标的原始 RGBA 像素。
const IDLE_RGBA: &[u8] = include_bytes!("../icons/idle.rgba");

/// 共享中（绿色手机带信号弧）图标的原始 RGBA 像素。
const ACTIVE_RGBA: &[u8] = include_bytes!("../icons/active.rgba");

/// 取得托盘图标的原始 RGBA 像素。
///
/// # 参数
/// - `active`: 是否为共享中样式。
///
/// # 返回值
/// `ICON_SIZE * ICON_SIZE * 4` 字节的非预乘 RGBA 数据。
///
/// # 示例
/// ```
/// use phone_netshare::icon::{icon_rgba, ICON_SIZE};
/// assert_eq!(icon_rgba(true).len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
/// ```
pub fn icon_rgba(active: bool) -> &'static [u8] {
    if active { ACTIVE_RGBA } else { IDLE_RGBA }
}

/// 创建适用于 [`tray_icon::TrayIcon`] 的原生图标对象。
///
/// # 参数
/// - `active`: 是否为共享中样式。
///
/// # 返回值
/// 图标对象 [`Icon`]。
///
/// # 示例
/// ```
/// use phone_netshare::icon::create_tray_icon_image;
/// let icon = create_tray_icon_image(false);
/// ```
pub fn create_tray_icon_image(active: bool) -> Icon {
    Icon::from_rgba(icon_rgba(active).to_vec(), ICON_SIZE, ICON_SIZE).expect("创建托盘图标失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两枚图标的像素长度都必须是 32*32*4。
    #[test]
    fn test_icon_pixel_length() {
        let expect = (ICON_SIZE * ICON_SIZE * 4) as usize;
        assert_eq!(icon_rgba(false).len(), expect);
        assert_eq!(icon_rgba(true).len(), expect);
    }

    /// 共享中和空闲的图标内容必须不同，且都有不透明像素。
    #[test]
    fn test_icon_states_differ_and_visible() {
        assert_ne!(icon_rgba(false), icon_rgba(true));
        assert!(icon_rgba(false).chunks(4).any(|p| p[3] == 255));
        assert!(icon_rgba(true).chunks(4).any(|p| p[3] == 255));
    }
}
