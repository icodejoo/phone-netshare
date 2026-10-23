//! gnirehtet 中继管理与手机网络共享控制模块。

use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::adb::{get_installed_version_code, install_and_grant, is_app_installed, test_sharing};
use crate::assets::{run_gnirehtet, CREATE_NO_WINDOW, STOP_APK_NAME};
use crate::logic::{Phone, RELAY_PORT, STOP_APP_PACKAGE, STOP_APP_VERSION_CODE};

/// 检测本机的 gnirehtet 中继服务是否已在监听（固定端口 31416）。
///
/// # 返回值
/// 若端口已被监听返回 `true`，未被占用返回 `false`。
///
/// # 示例
/// ```no_run
/// use phone_netshare::relay::is_relay_running;
/// let running = is_relay_running();
/// ```
pub fn is_relay_running() -> bool {
    // 尝试在 31416 端口绑定测试；若端口已在使用，说明中继正在运行
    match TcpListener::bind(("127.0.0.1", RELAY_PORT)) {
        Ok(_) => false,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => true,
        Err(_) => {
            // 补充快速连接检测，防止防火墙或网络策略特殊限制
            TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], RELAY_PORT)),
                Duration::from_millis(100),
            )
            .is_ok()
        }
    }
}

/// gnirehtet 中继进程管理器，负责按需启动与安全关闭自身启动的中继进程。
pub struct RelayManager {
    /// 自身启动的中继子进程句柄。
    child: Mutex<Option<Child>>,
}

impl Default for RelayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RelayManager {
    /// 创建新的中继进程管理器。
    ///
    /// # 返回值
    /// 初始状态未持有中继进程的 [`RelayManager`] 实例。
    ///
    /// # 示例
    /// ```
    /// use phone_netshare::relay::RelayManager;
    /// let mgr = RelayManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    /// 确保中继服务正在运行，若未运行则在后台隐藏启动一个中继进程并等待 800 毫秒。
    ///
    /// # 参数
    /// - `gnirehtet_path`: gnirehtet.exe 路径。
    /// - `adb_path`: adb.exe 路径。
    ///
    /// # 返回值
    /// 成功返回 `Ok(())`，失败返回错误描述。
    ///
    /// # 示例
    /// ```no_run
    /// use std::path::Path;
    /// use phone_netshare::relay::RelayManager;
    /// let mgr = RelayManager::new();
    /// mgr.ensure_relay_running(Path::new("gnirehtet.exe"), Path::new("adb.exe")).unwrap();
    /// ```
    pub fn ensure_relay_running(
        &self,
        gnirehtet_path: &Path,
        adb_path: &Path,
    ) -> Result<(), String> {
        if is_relay_running() {
            return Ok(());
        }

        let mut cmd = Command::new(gnirehtet_path);
        cmd.arg("relay");
        cmd.env("ADB", adb_path);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| format!("启动 gnirehtet relay 失败: {e}"))?;

        {
            let mut guard = self.child.lock().unwrap();
            *guard = Some(child);
        }

        std::thread::sleep(Duration::from_millis(800));
        Ok(())
    }

    /// 当没有手机处于网络共享状态时，关闭自身启动的中继进程（不影响外部中继）。
    ///
    /// # 参数
    /// - `adb_path`: adb 可执行文件路径。
    /// - `phones`: 当前所有已连接的手机列表。
    ///
    /// # 返回值
    /// 成功返回 `Ok(())`。
    ///
    /// # 示例
    /// ```no_run
    /// use std::path::Path;
    /// use phone_netshare::relay::RelayManager;
    /// let mgr = RelayManager::new();
    /// mgr.stop_relay_if_idle(Path::new("adb.exe"), &[]).unwrap();
    /// ```
    pub fn stop_relay_if_idle(&self, adb_path: &Path, phones: &[Phone]) -> Result<(), String> {
        let mut guard = self.child.lock().unwrap();
        let should_kill = match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => {
                    *guard = None;
                    return Ok(());
                }
                Ok(None) => true,
                Err(_) => false,
            },
            None => false,
        };

        if !should_kill {
            return Ok(());
        }

        for p in phones {
            if test_sharing(adb_path, &p.serial) {
                return Ok(());
            }
        }

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }

        Ok(())
    }

    /// 强制终止由当前管理器启动的中继进程。
    ///
    /// # 示例
    /// ```
    /// use phone_netshare::relay::RelayManager;
    /// let mgr = RelayManager::new();
    /// mgr.stop_relay_force();
    /// ```
    pub fn stop_relay_force(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
}

impl Drop for RelayManager {
    fn drop(&mut self) {
        self.stop_relay_force();
    }
}

/// 对指定手机开启网络共享（确保中继、必要时安装 App、启动客户端）。
///
/// # 参数
/// - `serial`: 手机序列号。
/// - `gnirehtet_path`: gnirehtet.exe 路径。
/// - `adb_path`: adb.exe 路径。
/// - `relay_mgr`: 中继服务管理器。
///
/// # 返回值
/// 成功返回 `Ok(())`，失败返回错误描述。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::relay::{RelayManager, enable_phone};
/// let mgr = RelayManager::new();
/// enable_phone("SN123", Path::new("gnirehtet.exe"), Path::new("adb.exe"), &mgr).unwrap();
/// ```
pub fn enable_phone(
    serial: &str,
    gnirehtet_path: &Path,
    adb_path: &Path,
    relay_mgr: &RelayManager,
) -> Result<(), String> {
    relay_mgr.ensure_relay_running(gnirehtet_path, adb_path)?;

    // 每次启用都探测手机有没有装 App，没装就自动安装
    if !is_app_installed(adb_path, serial) {
        let out = run_gnirehtet(gnirehtet_path, adb_path, &["install", serial])?;
        if !out.status.success() {
            return Err(format!(
                "自动安装 gnirehtet App 失败：{}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }

    // 「解除网络共享」小 App：未安装或版本与 exe 携带的不一致就强制覆盖；失败不影响共享本身
    let installed = get_installed_version_code(adb_path, serial, STOP_APP_PACKAGE);
    if installed != Some(STOP_APP_VERSION_CODE) {
        if let Some(dir) = gnirehtet_path.parent() {
            let _ = install_and_grant(adb_path, serial, &dir.join(STOP_APK_NAME), STOP_APP_PACKAGE);
        }
    }

    run_gnirehtet(gnirehtet_path, adb_path, &["start", serial])?;
    Ok(())
}

/// 对指定手机停止网络共享，若无其余手机在共享则自动关闭中继。
///
/// # 参数
/// - `serial`: 手机序列号。
/// - `gnirehtet_path`: gnirehtet.exe 路径。
/// - `adb_path`: adb.exe 路径。
/// - `relay_mgr`: 中继服务管理器。
/// - `all_phones`: 当前全部已连接手机列表。
///
/// # 返回值
/// 成功返回 `Ok(())`，失败返回错误描述。
///
/// # 示例
/// ```no_run
/// use std::path::Path;
/// use phone_netshare::relay::{RelayManager, disable_phone};
/// let mgr = RelayManager::new();
/// disable_phone("SN123", Path::new("gnirehtet.exe"), Path::new("adb.exe"), &mgr, &[]).unwrap();
/// ```
pub fn disable_phone(
    serial: &str,
    gnirehtet_path: &Path,
    adb_path: &Path,
    relay_mgr: &RelayManager,
    all_phones: &[Phone],
) -> Result<(), String> {
    run_gnirehtet(gnirehtet_path, adb_path, &["stop", serial])?;
    relay_mgr.stop_relay_if_idle(adb_path, all_phones)?;
    Ok(())
}
