//! 手机反向共享网络的核心数据转换与业务逻辑模块。

use std::collections::HashMap;

/// 空值在界面与信息列表中的占位符。
pub const EMPTY_PLACEHOLDER: &str = "—";

/// 共享中状态文本。
pub const STATE_TEXT_ON: &str = "共享中";

/// 未共享状态文本。
pub const STATE_TEXT_OFF: &str = "未共享";

/// gnirehtet 中继服务的默认监听端口。
pub const RELAY_PORT: u16 = 31416;

/// 手机端 App 包名。
pub const APP_PACKAGE: &str = "com.genymobile.gnirehtet";

/// 手机端虚拟隧道网卡名称。
pub const TUN_NAME: &str = "tun0";

/// 详情中读取的 Android 系统属性列表（属性键名与其对应的系统 getprop 键）。
pub const INFO_PROPS: &[(&str, &str)] = &[
    ("model", "ro.product.model"),
    ("brand", "ro.product.brand"),
    ("manufacturer", "ro.product.manufacturer"),
    ("marketname", "ro.product.marketname"),
    ("device", "ro.product.device"),
    ("release", "ro.build.version.release"),
    ("sdk", "ro.build.version.sdk"),
    ("abi", "ro.product.cpu.abi"),
];

/// 已连接的 Android 设备基本身份信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    /// 手机唯一硬件序列号。
    pub serial: String,
    /// 手机显示名称（缺省取型号，无型号时取序列号）。
    pub name: String,
}

/// 手机的详细系统与硬件信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneInfo {
    /// 设备名称（优先取自定义设备名，其次市场营销名，最后取型号）。
    pub name: String,
    /// 设备型号。
    pub model: String,
    /// 设备品牌。
    pub brand: String,
    /// 制造商名称。
    pub manufacturer: String,
    /// 设备硬件代号。
    pub device: String,
    /// 操作系统版本信息（形如 "Android 10 (SDK 29)"，无法获取时为占位符）。
    pub os: String,
    /// CPU 指令集架构。
    pub abi: String,
    /// 手机唯一序列号。
    pub serial: String,
}

/// 解析 `adb devices -l` 命令的文本输出，只保留已授权在线的设备。
///
/// # 参数
/// - `output`: `adb devices -l` 命令的完整标准输出内容。
///
/// # 返回值
/// 包含所有在线设备的列表，每台设备解析出序列号与型号名称。
///
/// # 示例
/// ```
/// use phone_netshare::logic::convert_from_adb_devices;
/// let text = "List of devices attached\n7NQBB23606003715 device model:STG_AL00\n";
/// let phones = convert_from_adb_devices(text);
/// assert_eq!(phones.len(), 1);
/// assert_eq!(phones[0].name, "STG_AL00");
/// ```
pub fn convert_from_adb_devices(output: &str) -> Vec<Phone> {
    let mut phones = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("List of devices") {
            continue;
        }

        // 匹配第一列序列号和后续的 "device" 状态
        let mut parts = trimmed.split_whitespace();
        let serial = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let status = match parts.next() {
            Some(s) => s,
            None => continue,
        };

        if status != "device" {
            continue;
        }

        // 查找 model:(\S+)
        let mut model = None;
        for part in parts {
            if let Some(stripped) = part.strip_prefix("model:") {
                if !stripped.is_empty() {
                    model = Some(stripped.to_string());
                    break;
                }
            }
        }

        let name = model.unwrap_or_else(|| serial.to_string());
        phones.push(Phone {
            serial: serial.to_string(),
            name,
        });
    }
    phones
}

/// 解析 `键=值` 形式的多行文本，每行仅按首个等号拆分。
///
/// # 参数
/// - `text`: 待解析的多行文本。
///
/// # 返回值
/// 包含已去首尾空白键值对的哈希表。
///
/// # 示例
/// ```
/// use phone_netshare::logic::convert_from_key_value_lines;
/// let kv = convert_from_key_value_lines("model=STG_AL00\nbrand=HUAWEI\n");
/// assert_eq!(kv.get("model").unwrap(), "STG_AL00");
/// ```
pub fn convert_from_key_value_lines(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim().to_string();
            let value = v.trim().to_string();
            if !key.is_empty() {
                map.insert(key, value);
            }
        }
    }
    map
}

/// 将读取到的系统属性映射整理成标准的手机详情信息。
///
/// # 参数
/// - `kv`: 键值属性字典。
/// - `serial`: 手机序列号。
///
/// # 返回值
/// 整理完成的 [`PhoneInfo`] 结构体。
///
/// # 示例
/// ```
/// use std::collections::HashMap;
/// use phone_netshare::logic::convert_to_phone_info;
/// let mut kv = HashMap::new();
/// kv.insert("model".to_string(), "STG_AL00".to_string());
/// let info = convert_to_phone_info(&kv, "123456");
/// assert_eq!(info.model, "STG_AL00");
/// ```
pub fn convert_to_phone_info(kv: &HashMap<String, String>, serial: &str) -> PhoneInfo {
    let get_prop = |key: &str| -> String {
        match kv.get(key) {
            Some(val) if !val.is_empty() && val != "null" => val.clone(),
            _ => EMPTY_PLACEHOLDER.to_string(),
        }
    };

    let device_name = get_prop("device_name");
    let marketname = get_prop("marketname");
    let model = get_prop("model");

    let name = if device_name != EMPTY_PLACEHOLDER {
        device_name
    } else if marketname != EMPTY_PLACEHOLDER {
        marketname
    } else if model != EMPTY_PLACEHOLDER {
        model.clone()
    } else {
        EMPTY_PLACEHOLDER.to_string()
    };

    let release = get_prop("release");
    let sdk = get_prop("sdk");
    let os = if release == EMPTY_PLACEHOLDER {
        EMPTY_PLACEHOLDER.to_string()
    } else {
        format!("Android {} (SDK {})", release, sdk)
    };

    PhoneInfo {
        name,
        model,
        brand: get_prop("brand"),
        manufacturer: get_prop("manufacturer"),
        device: get_prop("device"),
        os,
        abi: get_prop("abi"),
        serial: serial.to_string(),
    }
}

/// 为每台手机生成托盘菜单展示标签：同型号多台时追加序列号末 4 位。
///
/// # 参数
/// - `phones`: 当前检测到的手机列表。
///
/// # 返回值
/// 与输入同顺序的显示标签字符串列表。
///
/// # 示例
/// ```
/// use phone_netshare::logic::{Phone, get_phone_labels};
/// let list = vec![
///     Phone { serial: "A1001".into(), name: "Pixel".into() },
///     Phone { serial: "A1002".into(), name: "Pixel".into() },
/// ];
/// let labels = get_phone_labels(&list);
/// assert_eq!(labels[0], "Pixel (1001)");
/// assert_eq!(labels[1], "Pixel (1002)");
/// ```
pub fn get_phone_labels(phones: &[Phone]) -> Vec<String> {
    let mut labels = Vec::with_capacity(phones.len());
    for p in phones {
        let count = phones.iter().filter(|item| item.name == p.name).count();
        if count > 1 {
            let tail = if p.serial.len() > 4 {
                &p.serial[p.serial.len() - 4..]
            } else {
                &p.serial
            };
            labels.push(format!("{} ({})", p.name, tail));
        } else {
            labels.push(p.name.clone());
        }
    }
    labels
}

/// 拼装一台手机的全部详细信息文本（按行格式化为 `项目：值`，使用 CRLF 换行）。
///
/// # 参数
/// - `sharing`: 当前是否处于网络共享状态。
/// - `info`: 手机的详细属性信息。
///
/// # 返回值
/// 拼装好的多行文本字符串。
///
/// # 示例
/// ```
/// use phone_netshare::logic::{PhoneInfo, get_all_info_text, EMPTY_PLACEHOLDER};
/// let info = PhoneInfo {
///     name: "测试机".into(),
///     model: "DEMO-01".into(),
///     brand: "BRAND".into(),
///     manufacturer: "MFG".into(),
///     device: "DEV".into(),
///     os: "Android 14 (SDK 34)".into(),
///     abi: "arm64-v8a".into(),
///     serial: "SN123456".into(),
/// };
/// let text = get_all_info_text(true, &info);
/// assert!(text.starts_with("连接状态：共享中\r\n名称：测试机"));
/// ```
pub fn get_all_info_text(sharing: bool, info: &PhoneInfo) -> String {
    let state_text = if sharing {
        STATE_TEXT_ON
    } else {
        STATE_TEXT_OFF
    };
    let lines = [
        format!("连接状态：{state_text}"),
        format!("名称：{}", info.name),
        format!("型号：{}", info.model),
        format!("品牌：{}", info.brand),
        format!("制造商：{}", info.manufacturer),
        format!("设备代号：{}", info.device),
        format!("系统：{}", info.os),
        format!("CPU 架构：{}", info.abi),
        format!("序列号：{}", info.serial),
    ];
    lines.join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_from_adb_devices() {
        let sample = "\
List of devices attached
7NQBB23606003715       device product:STG-AL00 model:STG_AL00 device:HWSTG-Q transport_id:10
emulator-5554          device product:sdk model:sdk_gphone device:emu transport_id:3
R58M123ABC             unauthorized usb:1-1 transport_id:4
XYZ999                 offline transport_id:5
NOMODEL01              device transport_id:6
";
        let phones = convert_from_adb_devices(sample);
        assert_eq!(phones.len(), 3, "只保留 device 状态");
        assert_eq!(phones[0].serial, "7NQBB23606003715", "序列号取第一列");
        assert_eq!(phones[0].name, "STG_AL00", "型号取 model:");
        assert_eq!(phones[1].name, "sdk_gphone", "模拟器型号取 model:");
        assert_eq!(phones[2].name, "NOMODEL01", "没有 model: 时用序列号当名字");

        let empty_phones = convert_from_adb_devices("List of devices attached\n");
        assert_eq!(empty_phones.len(), 0, "没有设备时为空");
    }

    #[test]
    fn test_convert_from_key_value_lines() {
        let lines = "model=Pixel 7\r\nbrand=Google\r\nempty=\r\n=no_key\r\n";
        let kv = convert_from_key_value_lines(lines);
        assert_eq!(kv.get("model").map(String::as_str), Some("Pixel 7"));
        assert_eq!(kv.get("brand").map(String::as_str), Some("Google"));
        assert_eq!(kv.get("empty").map(String::as_str), Some(""));
        assert!(!kv.contains_key(""));
    }

    #[test]
    fn test_convert_to_phone_info() {
        let mut kv = HashMap::new();
        kv.insert("model".into(), "STG_AL00".into());
        kv.insert("brand".into(), "HUAWEI".into());
        kv.insert("manufacturer".into(), "HUAWEI".into());
        kv.insert("device".into(), "HWSTG".into());
        kv.insert("release".into(), "10".into());
        kv.insert("sdk".into(), "29".into());
        kv.insert("abi".into(), "arm64-v8a".into());
        kv.insert("marketname".into(), "null".into());
        kv.insert("device_name".into(), "".into());

        let info = convert_to_phone_info(&kv, "SN7NQBB");
        assert_eq!(info.name, "STG_AL00", "名称回退到 model");
        assert_eq!(info.os, "Android 10 (SDK 29)");
        assert_eq!(info.brand, "HUAWEI");
        assert_eq!(info.serial, "SN7NQBB");

        // 测试 release 为空时的系统版本占位
        let empty_info = convert_to_phone_info(&HashMap::new(), "SN000");
        assert_eq!(empty_info.os, EMPTY_PLACEHOLDER);
        assert_eq!(empty_info.name, EMPTY_PLACEHOLDER);
    }

    #[test]
    fn test_get_phone_labels() {
        let phones = vec![
            Phone {
                serial: "ABC001".into(),
                name: "Pixel".into(),
            },
            Phone {
                serial: "ABC002".into(),
                name: "Pixel".into(),
            },
            Phone {
                serial: "XYZ999".into(),
                name: "Galaxy".into(),
            },
        ];
        let labels = get_phone_labels(&phones);
        assert_eq!(labels[0], "Pixel (C001)");
        assert_eq!(labels[1], "Pixel (C002)");
        assert_eq!(labels[2], "Galaxy");
    }

    #[test]
    fn test_get_all_info_text() {
        let info = PhoneInfo {
            name: "我的手机".into(),
            model: "M1".into(),
            brand: "B1".into(),
            manufacturer: "MF1".into(),
            device: "D1".into(),
            os: "Android 14 (SDK 34)".into(),
            abi: "arm64".into(),
            serial: "S123".into(),
        };
        let text_on = get_all_info_text(true, &info);
        assert!(text_on.contains("连接状态：共享中"));
        assert!(text_on.contains("名称：我的手机"));
        assert!(text_on.contains("序列号：S123"));

        let text_off = get_all_info_text(false, &info);
        assert!(text_off.contains("连接状态：未共享"));
    }
}
