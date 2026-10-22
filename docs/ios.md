# iOS 支持现状

**结论：本项目暂不支持 iOS。**

gnirehtet 只有 Android 端（基于 `VpnService`），没有 iOS 版本，iOS 上也没有等价的「USB 反向共享」现成方案。要做的话是一个独立的大项目。

## 为什么难

| 环节 | Android（现在） | iOS（要自己做） |
|---|---|---|
| 手机端拦截全部流量 | `VpnService`，普通 App 即可，用户点一下授权 | **Network Extension 的 Packet Tunnel**，需要付费 Apple 开发者账号与对应 entitlement |
| 电脑到手机的 USB 通道 | `adb reverse` | usbmuxd（Windows 需要 Apple 驱动，即 iTunes 或 Apple Devices 应用） |
| 分发 | 官方 APK，adb 一条命令安装 | 签名限制：TestFlight、企业证书或自签（7 天过期），不能像 APK 那样随手装 |
| 开发环境 | 不需要 Mac | 编译和签名基本要 Mac 与 Xcode |

## 若以后要做，大致路线

1. iOS 端：写一个带 Packet Tunnel Provider 的 App，把隧道里的 IP 包经 usbmuxd 转到电脑。
2. 电脑端：写一个 usbmuxd 客户端，接收 IP 包并像 gnirehtet 中继那样做 TCP/UDP 代理。可参考 gnirehtet 的 Rust 中继实现（`relay-rust`）。
3. 托盘：沿用现有菜单，设备列表里增加 iOS 设备（通过 `idevice_id` 或 usbmuxd 枚举）。

在这些前置条件（开发者账号、Mac、驱动）具备之前，不建议立项。
