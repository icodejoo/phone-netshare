//! SHA256 哈希计算模块，基于 Windows 原生 CNG (BCrypt) API。

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::ptr;
use windows_sys::Win32::Security::Cryptography::{
    BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
    BCryptHashData, BCryptOpenAlgorithmProvider, BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE,
    BCRYPT_SHA256_ALGORITHM,
};

/// 把一段文本确定性地映射成 128 位整数（取其 SHA256 的前 128 位），可用作 GUID。
///
/// # 参数
/// - `text`: 输入文本，相同文本得到相同结果，不同文本几乎不会撞。
///
/// # 返回值
/// 成功返回 128 位整数，计算失败返回错误描述。
///
/// # 示例
/// ```
/// use phone_netshare::sha256::guid_from_text;
/// assert_eq!(guid_from_text("a").unwrap(), guid_from_text("a").unwrap());
/// assert_ne!(guid_from_text("a").unwrap(), guid_from_text("b").unwrap());
/// ```
pub fn guid_from_text(text: &str) -> Result<u128, String> {
    let hex = compute_sha256(text.as_bytes())?;
    u128::from_str_radix(&hex[..32], 16).map_err(|e| format!("解析哈希失败: {e}"))
}

/// 计算输入字节切片的 SHA256 哈希值，返回 64 位小写十六进制字符串。
///
/// # 参数
/// - `data`: 待计算哈希的字节切片。
///
/// # 返回值
/// 成功返回小写十六进制哈希字符串，失败返回错误描述。
///
/// # 示例
/// ```
/// let hash = phone_netshare::sha256::compute_sha256(b"hello").unwrap();
/// assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
/// ```
pub fn compute_sha256(data: &[u8]) -> Result<String, String> {
    unsafe {
        let mut alg_handle: BCRYPT_ALG_HANDLE = ptr::null_mut();
        let status = BCryptOpenAlgorithmProvider(
            &mut alg_handle,
            BCRYPT_SHA256_ALGORITHM,
            ptr::null(),
            0,
        );
        if status != 0 {
            return Err(format!("BCryptOpenAlgorithmProvider 失败: {status:#x}"));
        }

        let mut hash_handle: BCRYPT_HASH_HANDLE = ptr::null_mut();
        let status = BCryptCreateHash(
            alg_handle,
            &mut hash_handle,
            ptr::null_mut(),
            0,
            ptr::null(),
            0,
            0,
        );
        if status != 0 {
            let _ = BCryptCloseAlgorithmProvider(alg_handle, 0);
            return Err(format!("BCryptCreateHash 失败: {status:#x}"));
        }

        let status = BCryptHashData(hash_handle, data.as_ptr(), data.len() as u32, 0);
        if status != 0 {
            let _ = BCryptDestroyHash(hash_handle);
            let _ = BCryptCloseAlgorithmProvider(alg_handle, 0);
            return Err(format!("BCryptHashData 失败: {status:#x}"));
        }

        let mut hash_bytes = [0u8; 32];
        let status = BCryptFinishHash(hash_handle, hash_bytes.as_mut_ptr(), 32, 0);
        let _ = BCryptDestroyHash(hash_handle);
        let _ = BCryptCloseAlgorithmProvider(alg_handle, 0);

        if status != 0 {
            return Err(format!("BCryptFinishHash 失败: {status:#x}"));
        }

        let mut hex = String::with_capacity(64);
        for byte in hash_bytes {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{:02x}", byte);
        }
        Ok(hex)
    }
}

/// 校验指定文件的 SHA256 哈希是否与预期值一致（不区分大小写，自动去除预期值首尾空白）。
///
/// # 参数
/// - `path`: 文件路径。
/// - `expected_hex`: 期望的 SHA256 十六进制字符串。
///
/// # 返回值
/// 若文件存在且哈希一致返回 `true`，文件不存在或哈希不一致返回 `false`。
///
/// # 示例
/// ```no_run
/// let ok = phone_netshare::sha256::test_file_sha256("test.txt", "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
/// ```
pub fn test_file_sha256<P: AsRef<Path>>(path: P, expected_hex: &str) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buffer = Vec::new();
    if file.read_to_end(&mut buffer).is_err() {
        return false;
    }
    match compute_sha256(&buffer) {
        Ok(actual) => actual.eq_ignore_ascii_case(expected_hex.trim()),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_hello_sha256() {
        let data = b"hello";
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let actual = compute_sha256(data).expect("sha256 计算失败");
        assert_eq!(actual, expected, "sha256 计算结果不符合预期");
    }

    #[test]
    fn test_file_sha256_cases() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("netshare-test-{}.txt", std::process::id()));
        {
            let mut f = File::create(&temp_file).expect("创建临时文件失败");
            f.write_all(b"hello").expect("写入临时文件失败");
        }

        let hello_sha = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(test_file_sha256(&temp_file, hello_sha), "sha256 一致");
        assert!(test_file_sha256(&temp_file, &hello_sha.to_uppercase()), "sha256 不分大小写");
        let wrong_sha = "0".repeat(64);
        assert!(!test_file_sha256(&temp_file, &wrong_sha), "sha256 不一致");

        let _ = std::fs::remove_file(&temp_file);
    }

    /// 同一路径得到同一 GUID，不同路径得到不同 GUID。
    #[test]
    fn test_guid_from_text_stable_and_distinct() {
        let a = guid_from_text(r"c:\a\phone-netshare.exe").unwrap();
        let b = guid_from_text(r"c:\b\phone-netshare.exe").unwrap();
        assert_eq!(a, guid_from_text(r"c:\a\phone-netshare.exe").unwrap());
        assert_ne!(a, b);
    }
}
