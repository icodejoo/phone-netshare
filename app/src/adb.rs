//! adb 进程交互与 Android 设备通信模块。

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::assets::CREATE_NO_WINDOW;
use crate::logic::{
    convert_from_adb_devices, convert_from_key_value_lines, convert_to_phone_info, Phone,
    PhoneInfo, APP_PACKAGE, INFO_PROPS, STOP_APP_PERMISSION, TUN_NAME,
};

/// 找不到 adb 时的标准中文提示文案。
pub const MSG_ADB_NOT_FOUND: &str = "找不到 adb,请安装 Android platform-tools 并加入 PATH";

/// 从系统环境变量 `PATH` 中查找 `adb`（支持 `adb.exe` 与 `adb`）。
///
/// # 返回值
/// 找到时返回 adb 的完整路径 [`PathBuf`]，未找到返回 `None`。
///
/// # 示例
/// ```no_run
/// use phone_netshare::adb::find_adb_in_path;
/// if let Some(path) = find_adb_in_path() {
///     println!("找到 adb: {:?}", path);
/// }
/// ```
pub fn find_adb_in_path() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let exe_cand = dir.join("adb.exe");
        if exe_cand.is_file() {
            return Some(exe_cand);
        }
        let bin_cand = dir.join("adb");
        if bin_cand.is_file() {
            return Some(bin_cand);
        }
    }
    None
}

/// 执行指定的 adb 命令并获取其标准输出（自动附加 `CREATE_NO_WINDOW`）。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
/// - `args`: 命令行参数列表。
///
/// # 返回值
/// 成功返回去除首尾换行的标准输出文本，失败返回错误描述。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::run_adb;
/// let out = run_adb(Path::new("adb.exe"), &["version"]);
/// ```
pub fn run_adb(adb_path: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(adb_path);
    cmd.args(args);
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("运行 adb {:?} 失败: {e}", args))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

/// 获取当前连接且已授权的 Android 手机列表。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
///
/// # 返回值
/// 在线设备 [`Phone`] 列表。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::get_phones;
/// let phones = get_phones(Path::new("adb.exe")).unwrap_or_default();
/// ```
pub fn get_phones(adb_path: &Path) -> Result<Vec<Phone>, String> {
    let output = run_adb(adb_path, &["devices", "-l"])?;
    Ok(convert_from_adb_devices(&output))
}

/// 读取单台手机的系统属性及设备名称，整理为 [`PhoneInfo`]。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
/// - `serial`: 手机序列号。
///
/// # 返回值
/// 整理好的 [`PhoneInfo`] 结构体。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::get_phone_info;
/// let info = get_phone_info(Path::new("adb.exe"), "ABC12345");
/// ```
pub fn get_phone_info(adb_path: &Path, serial: &str) -> Result<PhoneInfo, String> {
    let mut shell_cmd = String::new();
    for (prop_key, getprop_key) in INFO_PROPS {
        shell_cmd.push_str(&format!("echo {prop_key}=$(getprop {getprop_key}); "));
    }
    shell_cmd.push_str("echo device_name=$(settings get global device_name)");

    let output = run_adb(adb_path, &["-s", serial, "shell", &shell_cmd])?;
    let kv = convert_from_key_value_lines(&output);
    Ok(convert_to_phone_info(&kv, serial))
}

/// 判断某台手机当前是否在共享电脑网络（检测手机端是否存在 tun0 网卡）。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
/// - `serial`: 手机序列号。
///
/// # 返回值
/// 若网卡存在则返回 `true`，否则返回 `false`。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::test_sharing;
/// let is_sharing = test_sharing(Path::new("adb.exe"), "ABC12345");
/// ```
pub fn test_sharing(adb_path: &Path, serial: &str) -> bool {
    let cmd = format!("ip -brief addr show {TUN_NAME} 2>/dev/null");
    match run_adb(adb_path, &["-s", serial, "shell", &cmd]) {
        Ok(out) => out.contains(TUN_NAME),
        Err(_) => false,
    }
}

/// 检查手机端是否已安装指定包名的应用。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
/// - `serial`: 手机序列号。
/// - `package`: 应用包名。
///
/// # 返回值
/// 若已安装返回 `true`，否则返回 `false`。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::is_package_installed;
/// let installed = is_package_installed(Path::new("adb.exe"), "ABC12345", "com.genymobile.gnirehtet");
/// ```
pub fn is_package_installed(adb_path: &Path, serial: &str, package: &str) -> bool {
    match run_adb(adb_path, &["-s", serial, "shell", "pm", "list", "packages", package]) {
        Ok(out) => out.lines().any(|l| l.trim() == format!("package:{package}")),
        Err(_) => false,
    }
}

/// 检查手机端是否已安装 gnirehtet 客户端应用。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
/// - `serial`: 手机序列号。
///
/// # 返回值
/// 若已安装返回 `true`，否则返回 `false`。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::is_app_installed;
/// let installed = is_app_installed(Path::new("adb.exe"), "ABC12345");
/// ```
pub fn is_app_installed(adb_path: &Path, serial: &str) -> bool {
    is_package_installed(adb_path, serial, APP_PACKAGE)
}

/// 给手机安装 APK，并授予小 App 所需的 `WRITE_SECURE_SETTINGS`。
///
/// # 参数
/// - `adb_path`: adb 可执行文件路径。
/// - `serial`: 手机序列号。
/// - `apk`: 本机 APK 路径。
/// - `package`: APK 的包名（授权用）。
///
/// # 返回值
/// 成功返回 `Ok(())`，安装或授权失败返回错误描述。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::adb::install_and_grant;
/// install_and_grant(Path::new("adb.exe"), "ABC12345", Path::new("phone-stop.apk"), "dev.jelon.netshare.stop").unwrap();
/// ```
pub fn install_and_grant(adb_path: &Path, serial: &str, apk: &Path, package: &str) -> Result<(), String> {
    let apk = apk.to_string_lossy();
    run_adb(adb_path, &["-s", serial, "install", "-r", &apk])?;
    run_adb(adb_path, &["-s", serial, "shell", "pm", "grant", package, STOP_APP_PERMISSION])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_constant() {
        assert!(MSG_ADB_NOT_FOUND.contains("找不到 adb"));
    }
}
