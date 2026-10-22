# 卸载：停掉托盘与中继，删桌面快捷方式；加 -RemoveBin 同时删除已下载的 gnirehtet。
param([switch]$RemoveBin)

$ErrorActionPreference = 'Stop'
# 仓库根目录
$Root = Split-Path -Parent $PSScriptRoot
# 桌面快捷方式路径
$Shortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Phone Network.lnk'

Get-CimInstance Win32_Process -Filter "Name='pwsh.exe'" | Where-Object CommandLine -Match 'phone-netshare-tray' |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Get-Process gnirehtet -ErrorAction SilentlyContinue | Stop-Process -Force
if (Test-Path -LiteralPath $Shortcut) { Remove-Item -LiteralPath $Shortcut }
if ($RemoveBin) { Remove-Item -LiteralPath (Join-Path $Root 'bin') -Recurse -Force -ErrorAction SilentlyContinue }
Write-Host '已卸载。手机上的 gnirehtet App 请自行在系统里卸载。'
