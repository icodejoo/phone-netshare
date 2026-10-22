# 安装：下载并校验 gnirehtet 官方发行包到 bin\，再在桌面建托盘快捷方式。
# 用法：pwsh -File scripts\install.ps1 [-NoShortcut]
param([switch]$NoShortcut)

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot '..\tray\NetShare.psm1') -Force

# 固定使用的 gnirehtet 版本
$Version = '2.5.1'
# 该版本 win64 包的 sha256（取自官方 SHA256SUMS.txt）
$ZipSha256 = '7f5b1063e7895182aa60def1437e50363c3758144088dcd079037bb7c3c46a1c'
# 官方发行包下载地址
$ZipUrl = "https://github.com/Genymobile/gnirehtet/releases/download/v$Version/gnirehtet-rust-win64-v$Version.zip"
# 仓库根目录与安装目录
$Root = Split-Path -Parent $PSScriptRoot
$Bin = Join-Path $Root 'bin'
# 桌面快捷方式名
$ShortcutName = 'Phone Network.lnk'

if (-not (Get-Command adb -ErrorAction SilentlyContinue)) { throw '找不到 adb：请先安装 Android platform-tools 并加入 PATH' }

if (-not (Test-Path (Join-Path $Bin 'gnirehtet.exe'))) {
    New-Item -ItemType Directory -Force $Bin | Out-Null
    $zip = Join-Path ([IO.Path]::GetTempPath()) "gnirehtet-$Version.zip"
    Write-Host "下载 $ZipUrl"
    Invoke-WebRequest -Uri $ZipUrl -OutFile $zip
    if (-not (Test-FileSha256 $zip $ZipSha256)) { Remove-Item -LiteralPath $zip; throw 'sha256 校验失败，已丢弃下载文件' }
    $tmp = Join-Path ([IO.Path]::GetTempPath()) "gnirehtet-$Version"
    Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force
    Get-ChildItem $tmp -Recurse -File | Where-Object Name -in 'gnirehtet.exe', 'gnirehtet.apk' | Copy-Item -Destination $Bin -Force
    Remove-Item -LiteralPath $zip, $tmp -Recurse -Force
}
Write-Host "gnirehtet 已就绪：$Bin"

if (-not $NoShortcut) {
    $ws = New-Object -ComObject WScript.Shell
    $lnk = $ws.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) $ShortcutName))
    $lnk.TargetPath = (Get-Command pwsh).Source
    $lnk.Arguments = "-NoProfile -WindowStyle Hidden -File `"$(Join-Path $Root 'tray\phone-netshare-tray.ps1')`""
    $lnk.WorkingDirectory = $Root
    $lnk.WindowStyle = 7
    $lnk.IconLocation = "$(Join-Path $Bin 'gnirehtet.exe'),0"
    $lnk.Description = '托盘：把电脑网络共享给 Android 手机'
    $lnk.Save()
    Write-Host "已在桌面创建快捷方式：$ShortcutName"
}
