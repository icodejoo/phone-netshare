# phone-netshare

让 Android 手机**通过 USB 直接用电脑的网络**（反向共享），Windows 托盘一键开关，支持多台手机。
底层是 [gnirehtet](https://github.com/Genymobile/gnirehtet)（Apache-2.0），本项目只做 Windows 端的安装、托盘和脚本，不自带也不修改它。

适合：手机没有可用的 Wi-Fi/流量，或者想让手机走电脑的网络环境（比如电脑上有代理、内网、特定出口）。

## 快速开始

前提：Windows 10/11、[PowerShell 7+](https://github.com/PowerShell/PowerShell)（`pwsh`）、`adb` 在 PATH 里、手机开了 USB 调试。

```powershell
pwsh -File scripts\install.ps1
```

脚本会：
1. 从官方 release 下载 gnirehtet v2.5.1（win64），**校验 sha256**，放到 `bin\`；
2. 在桌面创建 `Phone Network` 快捷方式。

双击快捷方式，右下角托盘出现图标。**左键或右键**图标：

| 菜单项 | 作用 |
|---|---|
| `✓ 型号 [序列号]` | 每台已连手机一行，点一下在「共享 / 停止」间切换，✓ 表示正在共享 |
| 全部启用 / 全部停止 | 对所有已连手机批量操作 |
| 退出 | 先停掉所有手机的共享，再关闭托盘 |

- 每次打开菜单都会重新扫描设备，新插拔的手机自动更新。
- 中继服务按需启动：第一台启用时才起，最后一台停止后自动关掉。
- 首次启用时，手机会弹出「VPN 连接请求」，点确定；手机端 App 缺失时会自动安装官方 APK。
- 重复双击快捷方式不会开出第二个托盘。

卸载：`pwsh -File scripts\uninstall.ps1`（加 `-RemoveBin` 同时删除已下载的 gnirehtet）。

## 目录

```
tray/NetShare.psm1               核心逻辑（设备解析、启停、中继管理），无界面
tray/phone-netshare-tray.ps1     托盘界面（WinForms NotifyIcon）
scripts/install.ps1              下载校验 gnirehtet、建桌面快捷方式
scripts/uninstall.ps1            卸载
tests/run-tests.ps1              零依赖单测（不需要手机）
docs/ios.md                      iOS 现状与路线
phone-stop/                      手机端「解除网络共享」小 App（纯 Java，约 12 KB）
scripts/build-stop-apk.sh        构建 phone-stop.apk（javac + d8 + aapt2，无需 Gradle）
```

## 自测与单测

```powershell
pwsh -File tests\run-tests.ps1                       # 单测，不需要手机
pwsh -File tray\phone-netshare-tray.ps1 -SelfTest    # 真机自测：对每台已连手机 启用→检查→停止
```

## 手机端程序

- **gnirehtet 官方 App**（`com.genymobile.gnirehtet`）：由 gnirehtet 在启用时用 adb 安装，本仓库不分发。
- **解除网络共享**（`dev.jelon.netshare.stop`，源码在 `phone-stop/`）：点手机上这个图标，就直接向 gnirehtet 发 STOP，不用再接电脑。
  - 已打包进托盘 exe：**每次启用共享时探测手机有没有装，没有就自动 `adb install` 并授予 `WRITE_SECURE_SETTINGS`**（发 STOP 必需，只有 adb 能授），无需手动操作。安装失败不影响共享本身。
  - 手动安装：`adb install bin/phone-stop.apk && adb shell pm grant dev.jelon.netshare.stop android.permission.WRITE_SECURE_SETTINGS`。
  - 本地构建：`bash scripts/build-stop-apk.sh`（需要 `ANDROID_HOME` 和 JDK）。签名 key 为仓库内固定的 `phone-stop/stop.keystore`（口令 `android`，仅保证覆盖安装，不作安全用途）。
  - CI 会先构建 APK 再嵌进 exe，推 `v*` 标签时 Release 同时附带 `phone-stop.apk`。

## 已知限制

- **只支持 Windows + Android**。iOS 见 [docs/ios.md](docs/ios.md)。
- gnirehtet 只转发 TCP/UDP，**不转发 ICMP**，手机上 `ping` 不通是正常的。
- 共享期间手机所有流量都走电脑出口。
- 同一台手机同时只能由一个中继接管；如果你另外开着 `gnirehtet run`，会和托盘抢端口 31416。

## 许可证

MIT，见 [LICENSE](LICENSE)。gnirehtet 的归属与许可见 [NOTICE](NOTICE)。
