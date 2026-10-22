//! 托盘图标：嵌入由 `assets/logo.png` 提取的多档透明度蒙版，运行时按系统托盘尺寸选档并着色。
//! 蒙版由 `scripts/export-icons.py` 导出，换 logo 后重新导出即可。

use tray_icon::Icon;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};

/// 嵌入的蒙版档位（边长，升序），须与导出脚本的 `SIZES` 一致。
pub const MASK_SIZES: [u32; 5] = [16, 20, 24, 32, 48];

/// 各档蒙版数据，下标与 [`MASK_SIZES`] 对应；每档 `边长*边长` 字节，仅 alpha。
const MASKS: [&[u8]; 5] = [
    include_bytes!("../icons/mask-16.a"),
    include_bytes!("../icons/mask-20.a"),
    include_bytes!("../icons/mask-24.a"),
    include_bytes!("../icons/mask-32.a"),
    include_bytes!("../icons/mask-48.a"),
];

/// 共享中的图形颜色（与 logo 浅蓝一致）。
const ACTIVE_RGB: [u8; 3] = [0x82, 0xDD, 0xFB];

/// 空闲时的图形颜色（中性灰）。
const IDLE_RGB: [u8; 3] = [0x8E, 0x93, 0x9A];

/// 按系统托盘图标尺寸挑选蒙版档位。
///
/// # 参数
/// - `want`: 期望边长（像素）。
///
/// # 返回值
/// 不小于 `want` 的最小档位下标；都比它小则取最大档。
///
/// # 示例
/// ```
/// use phone_netshare::icon::pick_mask_index;
/// assert_eq!(pick_mask_index(16), 0);
/// assert_eq!(pick_mask_index(22), 2);
/// assert_eq!(pick_mask_index(999), 4);
/// ```
pub fn pick_mask_index(want: u32) -> usize {
    MASK_SIZES.iter().position(|&s| s >= want).unwrap_or(MASK_SIZES.len() - 1)
}

/// 查询系统当前的小图标（托盘）边长。
///
/// # 返回值
/// 边长像素；查询失败时按 16 处理。
fn system_tray_icon_size() -> u32 {
    let v = unsafe { GetSystemMetrics(SM_CXSMICON) };
    if v > 0 { v as u32 } else { 16 }
}

/// 把 alpha 蒙版着色成 RGBA 像素。
///
/// # 参数
/// - `mask`: 每像素 1 字节的透明度。
/// - `rgb`: 图形颜色。
///
/// # 返回值
/// `mask.len() * 4` 字节的非预乘 RGBA 数据。
///
/// # 示例
/// ```
/// use phone_netshare::icon::tint_mask;
/// assert_eq!(tint_mask(&[0, 255], [1, 2, 3]), vec![1, 2, 3, 0, 1, 2, 3, 255]);
/// ```
pub fn tint_mask(mask: &[u8], rgb: [u8; 3]) -> Vec<u8> {
    mask.iter().flat_map(|&a| [rgb[0], rgb[1], rgb[2], a]).collect()
}

/// 取得指定档位、指定状态的图标像素。
///
/// # 参数
/// - `index`: 蒙版档位下标，须小于 [`MASK_SIZES`] 的长度。
/// - `active`: 是否为共享中样式（彩色）；否则为空闲样式（灰色）。
///
/// # 返回值
/// `边长 * 边长 * 4` 字节的非预乘 RGBA 数据。
///
/// # 示例
/// ```
/// use phone_netshare::icon::{icon_rgba, MASK_SIZES};
/// assert_eq!(icon_rgba(0, true).len(), (MASK_SIZES[0] * MASK_SIZES[0] * 4) as usize);
/// ```
pub fn icon_rgba(index: usize, active: bool) -> Vec<u8> {
    tint_mask(MASKS[index], if active { ACTIVE_RGB } else { IDLE_RGB })
}

/// 创建适用于 [`tray_icon::TrayIcon`] 的原生图标对象，尺寸自动匹配系统托盘。
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
    let index = pick_mask_index(system_tray_icon_size());
    let size = MASK_SIZES[index];
    Icon::from_rgba(icon_rgba(index, active), size, size).expect("创建托盘图标失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每档蒙版的字节数必须等于 边长*边长。
    #[test]
    fn test_mask_lengths() {
        for (i, &s) in MASK_SIZES.iter().enumerate() {
            assert_eq!(MASKS[i].len(), (s * s) as usize, "{s}px 蒙版长度不对");
        }
    }

    /// 每档蒙版都必须有不透明像素，图形不能是空的。
    #[test]
    fn test_masks_visible() {
        assert!(MASKS.iter().all(|m| m.iter().any(|&a| a > 200)));
    }

    /// 档位选择：取不小于期望值的最小档，超出则取最大档。
    #[test]
    fn test_pick_mask_index() {
        assert_eq!(pick_mask_index(0), 0);
        assert_eq!(pick_mask_index(16), 0);
        assert_eq!(pick_mask_index(17), 1);
        assert_eq!(pick_mask_index(32), 3);
        assert_eq!(pick_mask_index(1000), MASK_SIZES.len() - 1);
    }

    /// 共享中与空闲的颜色必须不同，但透明度逐像素一致。
    #[test]
    fn test_states_differ_in_color_only() {
        let (a, i) = (icon_rgba(0, true), icon_rgba(0, false));
        assert_ne!(a, i);
        assert!(a.chunks(4).zip(i.chunks(4)).all(|(x, y)| x[3] == y[3]));
    }
}
