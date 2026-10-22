#![windows_subsystem = "windows"]

//! phone-netshare Windows 托盘应用程序入口模块。

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::ptr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use muda::MenuEvent;
use tray_icon::TrayIconBuilder;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND};
use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Input::Ime::ImmDisableIME;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_GUID, NIF_INFO, NIIF_INFO, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage, TranslateMessage, MSG, WM_NULL,
};

use phone_netshare::adb::{
    find_adb_in_path, get_phone_info, get_phones, test_sharing, MSG_ADB_NOT_FOUND,
};
use phone_netshare::assets::ensure_gnirehtet_extracted;
use phone_netshare::icon::create_tray_icon_image;
use phone_netshare::logic::{get_phone_labels, Phone, PhoneInfo};
use phone_netshare::menu::{
    build_tray_menu, copy_text_to_clipboard, MenuAction, PhoneMenuEntry,
};
use phone_netshare::relay::{disable_phone, enable_phone, is_relay_running, RelayManager};
use phone_netshare::sha256::guid_from_text;

/// 按 GUID 清掉 Explorer 里可能残留的旧托盘图标。
///
/// 上一个进程被强杀或崩溃时，Explorer 会留下同 GUID 的幽灵图标，
/// 这时再 NIM_ADD 会失败且 tray-icon 不报错，表现为进程在跑、图标不显示。
fn remove_stale_tray_icon(guid: u128) {
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.uFlags = NIF_GUID;
        nid.guidItem = windows_sys::core::GUID::from_u128(guid);
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

/// 取不到 exe 路径时兜底用的托盘 GUID。
const FALLBACK_TRAY_GUID: u128 = 0x8a1b2c3d_4e5f_6a7b_8c9d_0e1f2a3b4c5d;

/// 托盘图标的 GUID，用于 Shell 标识与气泡弹窗绑定。
///
/// Windows 会把 GUID 和 exe 路径绑在一起，写死同一个 GUID 的话，exe 一换位置（便携版常见）
/// 注册就会静默失败、图标不显示。所以这里按 exe 的完整路径算，每个位置一个 GUID。
fn tray_guid() -> u128 {
    static GUID: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    *GUID.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| guid_from_text(&p.to_string_lossy().to_lowercase()).ok())
            .unwrap_or(FALLBACK_TRAY_GUID)
    })
}

/// 默认闲置状态下的轮询刷新间隔（毫秒）。
const IDLE_POLL_INTERVAL_MS: u64 = 10000;

/// 状态改变操作后的快速复查间隔（毫秒）。
const KICK_POLL_INTERVAL_MS: u64 = 2500;

/// 追加写入一行日志到 `%TEMP%\phone-netshare.log`。
///
/// # 参数
/// - `message`: 日志文本。
///
/// # 示例
/// ```no_run
/// write_netshare_log("初始化完成");
/// ```
fn write_netshare_log(message: &str) {
    let temp_log = std::env::temp_dir().join("phone-netshare.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(temp_log) {
        use std::time::SystemTime;
        let duration = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs() % 86400;
        let hours = (secs / 3600 + 8) % 24; // 东八区大致显示
        let mins = (secs % 3600) / 60;
        let s = secs % 60;
        let _ = writeln!(file, "{:02}:{:02}:{:02} {}", hours, mins, s, message);
    }
}

/// 弹出 Windows 任务栏系统托盘气泡通知。
///
/// # 参数
/// - `hwnd`: 托盘宿主窗口句柄。
/// - `title`: 通知标题。
/// - `text`: 通知正文。
///
/// # 示例
/// ```no_run
/// show_balloon_tip(std::ptr::null_mut(), "手机网络", "操作已完成");
/// ```
fn show_balloon_tip(hwnd: HWND, title: &str, text: &str) {
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uFlags = NIF_INFO | NIF_GUID;
        nid.guidItem = windows_sys::core::GUID::from_u128(tray_guid());
        nid.dwInfoFlags = NIIF_INFO;

        let title_wide: Vec<u16> = title.encode_utf16().take(63).collect();
        for (i, &c) in title_wide.iter().enumerate() {
            nid.szInfoTitle[i] = c;
        }

        let text_wide: Vec<u16> = text.encode_utf16().take(255).collect();
        for (i, &c) in text_wide.iter().enumerate() {
            nid.szInfo[i] = c;
        }

        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// 派发给后台工作线程执行的异步任务指令。
#[allow(dead_code)]
enum WorkerTask {
    /// 立即强制刷新扫描手机网络状态。
    ScanNow,
    /// 切换单台手机的共享状态。
    TogglePhone {
        serial: String,
        name: String,
        current_state: bool,
    },
    /// 对所有已连接手机开启网络共享。
    EnableAll,
    /// 对所有已连接手机停止网络共享。
    DisableAll,
    /// 关闭所有手机共享、清理中继并安全退出工作线程。
    Shutdown,
}

/// 后台工作线程向主界面线程报告的状态事件。
enum MainEvent {
    /// 手机状态列表及统计已更新。
    StateUpdated {
        adb_found: bool,
        entries: Vec<PhoneMenuEntry>,
        active_count: usize,
    },
    /// 请求弹出托盘气泡提示。
    ShowTip { text: String },
    /// 后台清理已全部完成，主线程可安全退出。
    ShutdownReady,
}

/// 执行自测逻辑（--self-test 模式）。
fn run_self_test() {
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let adb_path = match find_adb_in_path() {
        Some(path) => path,
        None => {
            println!("{MSG_ADB_NOT_FOUND}");
            write_netshare_log(MSG_ADB_NOT_FOUND);
            return;
        }
    };

    let gnirehtet_path = match ensure_gnirehtet_extracted() {
        Ok(path) => path,
        Err(e) => {
            let msg = format!("解压 gnirehtet 失败: {e}");
            println!("{msg}");
            write_netshare_log(&msg);
            return;
        }
    };

    let relay_mgr = RelayManager::new();
    let phones = match get_phones(&adb_path) {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("获取手机列表失败: {e}");
            println!("{msg}");
            write_netshare_log(&msg);
            return;
        }
    };

    if phones.is_empty() {
        let msg = "没有检测到手机";
        println!("{msg}");
        write_netshare_log(msg);
        return;
    }

    for p in &phones {
        let _ = enable_phone(&p.serial, &gnirehtet_path, &adb_path, &relay_mgr);
        thread::sleep(Duration::from_secs(3));
        let up = test_sharing(&adb_path, &p.serial);

        let _ = disable_phone(&p.serial, &gnirehtet_path, &adb_path, &relay_mgr, &phones);
        thread::sleep(Duration::from_secs(2));
        let down = test_sharing(&adb_path, &p.serial);
        let relay_on = is_relay_running();

        let line = format!(
            "{} [{}] 启用后隧道={} 停止后隧道={} 中继在跑={}",
            p.name, p.serial, up, down, relay_on
        );
        println!("{line}");
        write_netshare_log(&line);
    }
}

/// 后台扫描与业务执行工作线程入口。
fn start_worker_thread(
    gnirehtet_path: PathBuf,
    tx_main: Sender<MainEvent>,
    rx_task: Receiver<WorkerTask>,
    hwnd_holder: Arc<MutexHolder>,
) {
    let relay_mgr = RelayManager::new();
    let mut info_cache: HashMap<String, PhoneInfo> = HashMap::new();
    let mut cached_phones: Vec<Phone> = Vec::new();
    let mut next_poll_time = Instant::now();
    let mut poll_interval = Duration::from_millis(IDLE_POLL_INTERVAL_MS);
    let mut warned_adb_missing = false;

    loop {
        // 检查任务或按时超时扫描
        let now = Instant::now();
        let timeout = if next_poll_time > now {
            next_poll_time - now
        } else {
            Duration::from_millis(0)
        };

        match rx_task.recv_timeout(timeout) {
            Ok(WorkerTask::Shutdown) => {
                // 先实时重扫一遍手机（缓存可能是 10 秒前的快照），再逐台停止并关闭中继
                if let Some(adb) = find_adb_in_path() {
                    let phones = get_phones(&adb).unwrap_or_else(|_| cached_phones.clone());
                    for p in &phones {
                        let _ = disable_phone(&p.serial, &gnirehtet_path, &adb, &relay_mgr, &phones);
                    }
                }
                relay_mgr.stop_relay_force();
                let _ = tx_main.send(MainEvent::ShutdownReady);
                hwnd_holder.wake();
                break;
            }
            Ok(WorkerTask::TogglePhone {
                serial,
                name,
                current_state,
            }) => {
                if let Some(adb) = find_adb_in_path() {
                    if current_state {
                        let _ = disable_phone(
                            &serial,
                            &gnirehtet_path,
                            &adb,
                            &relay_mgr,
                            &cached_phones,
                        );
                        let _ = tx_main.send(MainEvent::ShowTip {
                            text: format!("{name} 已停止共享"),
                        });
                    } else {
                        let text = match enable_phone(&serial, &gnirehtet_path, &adb, &relay_mgr) {
                            Ok(()) => format!("{name} 已使用电脑网络"),
                            Err(e) => format!("{name} 启用失败：{e}"),
                        };
                        write_netshare_log(&text);
                        let _ = tx_main.send(MainEvent::ShowTip { text });
                    }
                    hwnd_holder.wake();
                }
                // 操作后设置 2.5 秒快速复查
                next_poll_time = Instant::now() + Duration::from_millis(KICK_POLL_INTERVAL_MS);
                poll_interval = Duration::from_millis(IDLE_POLL_INTERVAL_MS);
                continue;
            }
            Ok(WorkerTask::EnableAll) => {
                if let Some(adb) = find_adb_in_path() {
                    for p in &cached_phones {
                        let _ = enable_phone(&p.serial, &gnirehtet_path, &adb, &relay_mgr);
                    }
                    let _ = tx_main.send(MainEvent::ShowTip {
                        text: "已对所有手机启用".to_string(),
                    });
                    hwnd_holder.wake();
                }
                next_poll_time = Instant::now() + Duration::from_millis(KICK_POLL_INTERVAL_MS);
                poll_interval = Duration::from_millis(IDLE_POLL_INTERVAL_MS);
                continue;
            }
            Ok(WorkerTask::DisableAll) => {
                if let Some(adb) = find_adb_in_path() {
                    for p in &cached_phones {
                        let _ = disable_phone(
                            &p.serial,
                            &gnirehtet_path,
                            &adb,
                            &relay_mgr,
                            &cached_phones,
                        );
                    }
                    let _ = tx_main.send(MainEvent::ShowTip {
                        text: "已全部停止".to_string(),
                    });
                    hwnd_holder.wake();
                }
                next_poll_time = Instant::now() + Duration::from_millis(KICK_POLL_INTERVAL_MS);
                poll_interval = Duration::from_millis(IDLE_POLL_INTERVAL_MS);
                continue;
            }
            Ok(WorkerTask::ScanNow) => {
                // 立即执行扫描
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // 定时唤醒执行扫描
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }

        // 执行设备与状态扫描
        let adb_opt = find_adb_in_path();
        if let Some(adb) = adb_opt {
            warned_adb_missing = false;
            let phones = get_phones(&adb).unwrap_or_default();
            let labels = get_phone_labels(&phones);

            let mut entries = Vec::with_capacity(phones.len());
            let mut active_count = 0;

            for (i, p) in phones.iter().enumerate() {
                let info = match info_cache.get(&p.serial) {
                    Some(cached) => cached.clone(),
                    None => {
                        let fetched = get_phone_info(&adb, &p.serial).unwrap_or_else(|_| PhoneInfo {
                            name: p.name.clone(),
                            model: p.name.clone(),
                            brand: "—".into(),
                            manufacturer: "—".into(),
                            device: "—".into(),
                            os: "—".into(),
                            abi: "—".into(),
                            serial: p.serial.clone(),
                        });
                        info_cache.insert(p.serial.clone(), fetched.clone());
                        fetched
                    }
                };

                let sharing = test_sharing(&adb, &p.serial);
                if sharing {
                    active_count += 1;
                }

                entries.push(PhoneMenuEntry {
                    serial: p.serial.clone(),
                    label: labels.get(i).cloned().unwrap_or_else(|| p.name.clone()),
                    sharing,
                    info: Some(info),
                });
            }

            cached_phones = phones;

            let _ = tx_main.send(MainEvent::StateUpdated {
                adb_found: true,
                entries,
                active_count,
            });
            hwnd_holder.wake();
        } else {
            cached_phones.clear();
            if !warned_adb_missing {
                warned_adb_missing = true;
                let _ = tx_main.send(MainEvent::ShowTip {
                    text: MSG_ADB_NOT_FOUND.to_string(),
                });
            }
            let _ = tx_main.send(MainEvent::StateUpdated {
                adb_found: false,
                entries: Vec::new(),
                active_count: 0,
            });
            hwnd_holder.wake();
        }

        next_poll_time = Instant::now() + poll_interval;
    }
}

/// 包装安全共享的 HWND，用于跨线程唤醒主线程消息队列。
struct MutexHolder {
    /// 宿主窗口句柄。
    hwnd: std::sync::atomic::AtomicIsize,
}

impl MutexHolder {
    /// 创建新的空句柄包装器。
    fn new() -> Self {
        Self {
            hwnd: std::sync::atomic::AtomicIsize::new(0),
        }
    }

    /// 设置实际的 HWND 句柄值。
    fn set(&self, hwnd: HWND) {
        self.hwnd
            .store(hwnd as isize, std::sync::atomic::Ordering::SeqCst);
    }

    /// 向主窗口投递空消息以唤醒 GetMessageW。
    fn wake(&self) {
        let val = self.hwnd.load(std::sync::atomic::Ordering::SeqCst);
        if val != 0 {
            unsafe {
                PostMessageW(val as HWND, WM_NULL, 0, 0);
            }
        }
    }
}

fn main() {
    // 托盘程序没有文本输入，尽早对本进程关闭输入法，避免输入法 DLL 被注入占内存
    unsafe {
        ImmDisableIME(u32::MAX);
    }

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--self-test") {
        run_self_test();
        return;
    }

    // 命名互斥体单实例检查
    let mutex_name: Vec<u16> = "Local\\phone-netshare-tray\0".encode_utf16().collect();
    let mutex_handle = unsafe { CreateMutexW(ptr::null(), 0, mutex_name.as_ptr()) };
    if mutex_handle.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }

    // 确保 gnirehtet.exe 解压就绪
    let gnirehtet_path = match ensure_gnirehtet_extracted() {
        Ok(path) => path,
        Err(e) => {
            write_netshare_log(&format!("解压 gnirehtet 失败: {e}"));
            return;
        }
    };

    // 预备空闲与共享中的图标
    let icon_idle = create_tray_icon_image(false);
    let icon_active = create_tray_icon_image(true);

    // 初始菜单构建
    let initial_adb_found = find_adb_in_path().is_some();
    let (initial_menu, mut action_map) = build_tray_menu(initial_adb_found, &[]);

    // 先清掉可能残留的幽灵图标，再创建系统托盘图标
    remove_stale_tray_icon(tray_guid());
    let tray_icon = match TrayIconBuilder::new()
        .with_guid(tray_guid())
        .with_icon(icon_idle.clone())
        .with_tooltip("手机网络：未共享")
        .with_menu(Box::new(initial_menu))
        .with_menu_on_left_click(true)
        .with_menu_on_right_click(true)
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            write_netshare_log(&format!("创建托盘图标失败: {e}"));
            return;
        }
    };

    let hwnd = tray_icon.window_handle();
    let hwnd_holder = Arc::new(MutexHolder::new());
    hwnd_holder.set(hwnd);

    // 初始 adb 缺失提醒
    if !initial_adb_found {
        show_balloon_tip(hwnd, "手机网络", MSG_ADB_NOT_FOUND);
    }

    // 启动后台工作线程
    let (tx_main, rx_main) = channel::<MainEvent>();
    let (tx_task, rx_task) = channel::<WorkerTask>();
    let worker_hwnd_holder = hwnd_holder.clone();

    thread::spawn(move || {
        start_worker_thread(gnirehtet_path, tx_main, rx_task, worker_hwnd_holder);
    });

    // 上一次建菜单时的状态指纹，用来判断菜单要不要重建
    let mut last_menu_sig = String::new();

    // Windows 消息循环
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        'main_loop: while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);

            // 处理后台 Worker 发送过来的事件
            while let Ok(event) = rx_main.try_recv() {
                match event {
                    MainEvent::StateUpdated {
                        adb_found,
                        entries,
                        active_count,
                    } => {
                        // 更新图标样式
                        let target_icon = if active_count > 0 {
                            icon_active.clone()
                        } else {
                            icon_idle.clone()
                        };
                        let _ = tray_icon.set_icon(Some(target_icon));

                        // 更新悬停提示
                        let tooltip = if active_count > 0 {
                            format!("手机网络：{active_count} 台共享中")
                        } else {
                            "手机网络：未共享".to_string()
                        };
                        let _ = tray_icon.set_tooltip(Some(tooltip));

                        // 内容有变化才重建菜单：重建会换掉所有菜单项 ID，
                        // 用户正点的那一下如果撞上重建，旧 ID 查不到就被丢弃（表现为"要点好几次"）
                        let sig = format!("{adb_found}|{entries:?}");
                        if sig != last_menu_sig {
                            let (menu, new_map) = build_tray_menu(adb_found, &entries);
                            action_map = new_map;
                            tray_icon.set_menu(Some(Box::new(menu)));
                            last_menu_sig = sig;
                        }
                    }
                    MainEvent::ShowTip { text } => {
                        show_balloon_tip(hwnd, "手机网络", &text);
                    }
                    MainEvent::ShutdownReady => {
                        // 后台完全清理完毕，退出消息循环
                        PostQuitMessage(0);
                        break 'main_loop;
                    }
                }
            }

            // 处理来自 muda 的菜单点击事件
            while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                if action_map.get(&menu_event.id).is_none() {
                    write_netshare_log(&format!("点击了未登记的菜单项 id={:?}（菜单可能刚被重建）", menu_event.id));
                }
                if let Some(action) = action_map.get(&menu_event.id).cloned() {
                    match action {
                        MenuAction::ToggleSharing {
                            serial,
                            name,
                            current_state,
                        } => {
                            write_netshare_log(&format!("收到切换点击：{name}（当前共享={current_state}）"));
                            let _ = tx_task.send(WorkerTask::TogglePhone {
                                serial,
                                name,
                                current_state,
                            });
                        }
                        MenuAction::CopyText { label, value } => {
                            if copy_text_to_clipboard(&value).is_ok() {
                                show_balloon_tip(hwnd, "手机网络", &format!("已复制{label}：{value}"));
                            }
                        }
                        MenuAction::CopyAllInfo { serial: _, text } => {
                            if copy_text_to_clipboard(&text).is_ok() {
                                show_balloon_tip(hwnd, "手机网络", "已复制全部信息");
                            }
                        }
                        MenuAction::EnableAll => {
                            let _ = tx_task.send(WorkerTask::EnableAll);
                        }
                        MenuAction::DisableAll => {
                            let _ = tx_task.send(WorkerTask::DisableAll);
                        }
                        MenuAction::Quit => {
                            // 发送关闭指令给 worker，等待其停止所有共享后发回 ShutdownReady
                            let _ = tx_task.send(WorkerTask::Shutdown);
                        }
                    }
                }
            }
        }
    }

    // 隐藏托盘图标
    let _ = tray_icon.set_visible(false);
}
