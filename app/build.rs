//! 构建脚本：把 `assets/logo.ico` 嵌进 exe 资源段，让资源管理器/任务栏显示 logo。

fn main() {
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
