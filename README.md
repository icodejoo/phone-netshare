# phone-netshare

让 Android 手机**通过 USB 直接用电脑的网络**（反向共享），Windows 托盘一键开关，支持多台手机。
底层是 [gnirehtet](https://github.com/Genymobile/gnirehtet)（Apache-2.0），本项目只做 Windows 端的托盘、打包和一个手机端小 App，不修改 gnirehtet。

适合：手机没有可用的 Wi-Fi/流量，或者想让手机走电脑的网络环境（比如电脑上有代理、内网、特定出口）。

## 快速开始

前提：Windows 10/11、`adb`（Android platform-tools）在 PATH 里、手机开了 USB 调试。

1. 到 [Releases](../../releases) 下载 `phone-netshare.exe`，双击运行。
2. 右下角托盘出现图标，**左键或右键**点开菜单。

exe 是**自包含的便携版**：gnirehtet 本体、gnirehtet 的手机 App、「解除网络共享」小 App 都已内嵌，首次运行会释放到 `%LOCALAPPDATA%\phone-netshare\`（SHA256 一致则跳过），不需要另外下载。

### 托盘菜单

| 菜单项 | 作用 |
|---|---|
| 每台已连手机一行（共享中前面打勾） | 点开二级菜单 |
| ├ 启用共享 / 停用共享 | 在两种状态间切换，共享中打勾 |
| ├ 设备名、型号、品牌、制造商、设备代号、系统、CPU 架构、序列号 | 点某一行复制该项 |
| └ 复制全部信息 | 复制上面全部内容 |
| 全部启用 / 全部停止 | 对所有已连手机批量操作 |
| 退出 | 先停掉所有手机的共享，再关闭托盘 |

- 每次打开菜单都会重新扫描设备，新插拔的手机自动更新。
- 托盘图标按系统 DPI 选档着色：空闲灰、有手机在共享时蓝；操作结果用气泡通知提示。
- 中继服务按需启动：第一台启用时才起，最后一台停止后自动关掉。
- 首次启用时，手机会弹出「VPN 连接请求」，点确定。
- 重复双击不会开出第二个托盘（命名互斥体单实例）。
- 排错日志在 `%TEMP%\phone-netshare.log`。

## 手机端程序

每次启用共享时，exe 会检查手机上的两个 App，缺了或版本不对就用 adb 自动处理：

| App | 包名 | 来源 | 用途 |
|---|---|---|---|
| gnirehtet 官方 App | `com.genymobile.gnirehtet` | exe 内嵌的官方 APK，缺失时 `gnirehtet install` | 建立 VPN 隧道，转发流量到电脑 |
| 解除网络共享 | `dev.jelon.netshare.stop` | 本仓库 `phone-stop/`，纯 Java，约 12 KB | 点手机上的图标，直接向 gnirehtet 发 STOP，不用再接电脑 |

「解除网络共享」小 App 的细节：

- **自动安装并授权**：未安装时 `adb install`，并授予 `WRITE_SECURE_SETTINGS`（gnirehtet 的 STOP 入口要这个权限，只有 adb 能授）。安装失败不影响共享本身。
- **版本强制对齐**：版本以 `phone-stop/AndroidManifest.xml` 的 `versionCode` 为准，编译时写进 exe。手机上的版本与之不一致（含更高）就覆盖安装；覆盖失败（如签名不同）则卸载重装。**改了小 App 记得递增 `versionCode`**。
- **手动安装**：`adb install bin/phone-stop.apk && adb shell pm grant dev.jelon.netshare.stop android.permission.WRITE_SECURE_SETTINGS`。
- **签名**：用仓库内固定的 `phone-stop/stop.keystore`（口令 `android`），只为保证每次构建签名一致、能覆盖安装，不作安全用途。

## 从源码构建

托盘 exe（Rust，`app/`）：

```powershell
# 1. 准备内嵌资源到 app/assets/（不入库）：
#    gnirehtet.exe、gnirehtet.apk 取自官方发行包 v2.5.1（CI 里会校验 sha256，见 .github/workflows/build.yml）
bash scripts/build-stop-apk.sh                       # 2. 构建小 App（需 ANDROID_HOME 和 JDK）
Copy-Item bin\phone-stop.apk app\assets\             #    放进 exe 的内嵌目录
cd app; cargo test; cargo build --release            # 3. 单测 + release 构建
```

- 真机自测（对每台已连手机 启用→检查→停止）：`phone-netshare.exe --self-test`。
- CI（`.github/workflows/build.yml`）：下载并校验 gnirehtet → 构建小 App APK → 单测 → release 构建 → 上传产物；推 `v*` 标签时发 Release，附带 `phone-netshare.exe` 和 `phone-stop.apk`。

## 目录

```
app/                             Rust 托盘程序（便携 exe）
  src/main.rs                    托盘、消息循环、后台线程
  src/menu.rs                    菜单构建
  src/relay.rs                   中继管理、启用/停用手机、小 App 安装与版本对齐
  src/adb.rs                     adb 调用（设备、信息、安装、版本查询）
  src/assets.rs                  内嵌资源释放与 SHA256 校验
  src/logic.rs                   设备解析、常量、版本号解析（含单测）
  icons/                         托盘图标蒙版（由 scripts/export-icons.py 生成）
phone-stop/                      手机端「解除网络共享」小 App（纯 Java）
scripts/build-stop-apk.sh        构建 phone-stop.apk（javac + d8 + aapt2，无需 Gradle）
scripts/export-icons.py          从 logo 生成托盘图标蒙版
assets/                          logo（svg/png/ico）
docs/ios.md                      iOS 现状与路线
tray/、scripts/install.ps1、tests/run-tests.ps1   早期 PowerShell 版托盘（见下）
```

## 早期 PowerShell 版

仓库里还保留着最初的 PowerShell 托盘，功能是 exe 的子集，**不含「解除网络共享」小 App**。需要 PowerShell 7+（`pwsh`）。

```powershell
pwsh -File scripts\install.ps1                       # 下载校验 gnirehtet 到 bin\，在桌面建 Phone Network 快捷方式
pwsh -File tests\run-tests.ps1                       # 单测，不需要手机
pwsh -File tray\phone-netshare-tray.ps1 -SelfTest    # 真机自测
pwsh -File scripts\uninstall.ps1                     # 卸载（加 -RemoveBin 同时删除已下载的 gnirehtet）
```

## 已知限制

- **只支持 Windows + Android**。iOS 见 [docs/ios.md](docs/ios.md)。
- gnirehtet 只转发 TCP/UDP，**不转发 ICMP**，手机上 `ping` 不通是正常的。
- 共享期间手机所有流量都走电脑出口。
- 同一台手机同时只能由一个中继接管；如果你另外开着 `gnirehtet run`，会和托盘抢端口 31416。
- 「解除网络共享」依赖 adb 授权的 `WRITE_SECURE_SETTINGS`；手机上卸载重装该 App 后需要再连一次电脑（exe 会在下次启用时自动补授权）。

## 许可证

MIT，见 [LICENSE](LICENSE)。gnirehtet 的归属与许可见 [NOTICE](NOTICE)。
