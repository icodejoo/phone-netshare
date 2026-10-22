//! 内嵌资源管理模块，负责 gnirehtet.exe 的解压与哈希校验。

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::sha256::{compute_sha256, test_file_sha256};

/// Windows 子进程无窗口创建标志。
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 编译期内嵌的 gnirehtet.exe 二进制数据。
pub const GNIREHTET_BYTES: &[u8] = include_bytes!("../assets/gnirehtet.exe");

/// 编译期内嵌的 gnirehtet.apk（手机端 App，启用时检测到没装就由 gnirehtet install 推送）。
pub const GNIREHTET_APK_BYTES: &[u8] = include_bytes!("../assets/gnirehtet.apk");

/// 获取内嵌 gnirehtet.exe 的只读字节切片。
///
/// # 返回值
/// 静态生命周期的二进制字节切片。
///
/// # 示例
/// ```
/// use phone_netshare::assets::get_gnirehtet_bytes;
/// assert!(!get_gnirehtet_bytes().is_empty());
/// ```
pub fn get_gnirehtet_bytes() -> &'static [u8] {
    GNIREHTET_BYTES
}

/// 确保 gnirehtet.exe 已释放到 `%LOCALAPPDATA%\phone-netshare\`，SHA256 一致时跳过写入。
///
/// # 返回值
/// 成功返回已就绪可执行文件的完整路径，失败返回错误描述字符串。
///
/// # 示例
/// ```no_run
/// use phone_netshare::assets::ensure_gnirehtet_extracted;
/// let path = ensure_gnirehtet_extracted().expect("解压 gnirehtet 失败");
/// assert!(path.exists());
/// ```
pub fn ensure_gnirehtet_extracted() -> Result<PathBuf, String> {
    let local_app_data = match std::env::var_os("LOCALAPPDATA") {
        Some(val) => PathBuf::from(val),
        None => {
            let user_profile = std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            user_profile.join("AppData").join("Local")
        }
    };

    let target_dir = local_app_data.join("phone-netshare");

    // apk 必须和 exe 同目录，gnirehtet install 才找得到
    extract_if_changed(&target_dir, "gnirehtet.apk", GNIREHTET_APK_BYTES)?;
    extract_if_changed(&target_dir, "gnirehtet.exe", GNIREHTET_BYTES)
}

/// 把内嵌字节写到目录下的指定文件；文件已存在且 SHA256 一致则跳过写入。
///
/// # 参数
/// - `dir`: 目标目录，不存在会自动创建。
/// - `name`: 目标文件名。
/// - `bytes`: 要写入的内容。
///
/// # 返回值
/// 成功返回文件完整路径，失败返回错误描述字符串。
fn extract_if_changed(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let target_file = dir.join(name);
    let expected_hash = compute_sha256(bytes)?;

    if target_file.exists() && test_file_sha256(&target_file, &expected_hash) {
        return Ok(target_file);
    }

    fs::create_dir_all(dir).map_err(|e| format!("创建目录 {:?} 失败: {e}", dir))?;
    fs::write(&target_file, bytes).map_err(|e| format!("写入文件 {:?} 失败: {e}", target_file))?;
    Ok(target_file)
}

/// 执行 gnirehtet 命令行，并为其注入 `ADB` 环境变量和隐藏窗口标志。
///
/// # 参数
/// - `gnirehtet_path`: gnirehtet.exe 的完整文件路径。
/// - `adb_path`: PATH 中找到的 adb.exe 完整路径。
/// - `args`: 传递给 gnirehtet 的子命令及参数列表。
///
/// # 返回值
/// 进程执行的输出结果 [`Output`]。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::assets::run_gnirehtet;
/// let out = run_gnirehtet(Path::new("gnirehtet.exe"), Path::new("adb.exe"), &["--version"]);
/// ```
pub fn run_gnirehtet(
    gnirehtet_path: &Path,
    adb_path: &Path,
    args: &[&str],
) -> Result<Output, String> {
    let mut cmd = Command::new(gnirehtet_path);
    cmd.args(args);
    cmd.env("ADB", adb_path);
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.output()
        .map_err(|e| format!("运行 gnirehtet {:?} 失败: {e}", args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_bytes_not_empty() {
        assert!(!GNIREHTET_BYTES.is_empty(), "内嵌 gnirehtet.exe 字节不能为空");
        let hash = compute_sha256(GNIREHTET_BYTES).expect("计算内嵌文件哈希失败");
        assert_eq!(hash.len(), 64, "SHA256 哈希应为 64 位字符");
    }
}
