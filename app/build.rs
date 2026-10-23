//! 构建脚本：把 `assets/logo.ico` 嵌进 exe 资源段，让资源管理器/任务栏显示 logo。

/// 从 phone-stop 的清单里取 `versionCode`，作为 exe 携带的小 App 版本（与内嵌 APK 同源，不会漂移）。
fn stop_app_version_code() -> String {
    let manifest = std::fs::read_to_string("../phone-stop/AndroidManifest.xml").expect("读取 phone-stop 清单失败");
    let rest = manifest.split("android:versionCode=\"").nth(1).expect("清单缺少 versionCode");
    rest.split('"').next().expect("versionCode 格式错误").to_string()
}

fn main() {
    println!("cargo:rerun-if-changed=../phone-stop/AndroidManifest.xml");
    println!("cargo:rustc-env=STOP_APP_VERSION_CODE={}", stop_app_version_code());
    println!("cargo:rerun-if-changed=../assets/logo.ico");
    println!("cargo:rerun-if-changed=build.rs");
    // 资源嵌入只在 Windows 目标上有意义
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../assets/logo.ico");
        res.set("ProductName", "phone-netshare");
        res.set("FileDescription", "手机网络共享托盘");
        res.compile().expect("嵌入 exe 图标失败");
    }
}
