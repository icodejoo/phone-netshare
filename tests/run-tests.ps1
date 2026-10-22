# 零依赖单测：不需要手机，也不需要 Pester。用法：pwsh -File tests\run-tests.ps1 ，失败时退出码为 1。
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot '..\tray\NetShare.psm1') -Force

# 失败计数
$script:Failed = 0

<# .SYNOPSIS 断言两个值相等，不等就记一次失败并打印。 #>
function Assert-Equal($Actual, $Expected, [string]$Name) {
    if ($Actual -ceq $Expected) { Write-Host "通过  $Name" }
    else { $script:Failed++; Write-Host "失败  $Name`n      期望：$Expected`n      实际：$Actual" -ForegroundColor Red }
}

# --- ConvertFrom-AdbDevices ---
$sample = @(
    'List of devices attached',
    '7NQBB23606003715       device product:STG-AL00 model:STG_AL00 device:HWSTG-Q transport_id:10',
    'emulator-5554          device product:sdk model:sdk_gphone device:emu transport_id:3',
    'R58M123ABC             unauthorized usb:1-1 transport_id:4',
    'XYZ999                 offline transport_id:5',
    'NOMODEL01              device transport_id:6',
    ''
)
$phones = @(ConvertFrom-AdbDevices $sample)
Assert-Equal $phones.Count 3 '只保留 device 状态（过滤 unauthorized / offline / 表头 / 空行）'
Assert-Equal $phones[0].Serial '7NQBB23606003715' '序列号取第一列'
Assert-Equal $phones[0].Name 'STG_AL00' '型号取 model:'
Assert-Equal $phones[2].Name 'NOMODEL01' '没有 model: 时用序列号当名字'
Assert-Equal @(ConvertFrom-AdbDevices @('List of devices attached', '')).Count 0 '没有设备时为空'

# --- Test-FileSha256 ---
$tmp = Join-Path ([IO.Path]::GetTempPath()) "netshare-test-$([guid]::NewGuid()).txt"
Set-Content -LiteralPath $tmp -Value 'hello' -NoNewline -Encoding ascii
# 'hello' 的 sha256
$helloSha = '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824'
Assert-Equal (Test-FileSha256 $tmp $helloSha) $true 'sha256 一致'
Assert-Equal (Test-FileSha256 $tmp $helloSha.ToUpper()) $true 'sha256 不分大小写'
Assert-Equal (Test-FileSha256 $tmp ('0' * 64)) $false 'sha256 不一致'

# --- Get-GnirehtetExe ---
$env:PHONE_NETSHARE_EXE = $tmp
Assert-Equal (Get-GnirehtetExe) (Resolve-Path -LiteralPath $tmp).Path '环境变量优先'
$env:PHONE_NETSHARE_EXE = Join-Path ([IO.Path]::GetTempPath()) 'not-exist-gnirehtet.exe'
$threw = $false
try { $null = Get-GnirehtetExe } catch { $threw = $true }
# 环境变量指向不存在的文件、仓库 bin 与 PATH 里都没有时应抛异常；本机若装了 gnirehtet 则跳过这条
if (-not (Get-Command gnirehtet.exe -ErrorAction SilentlyContinue) -and -not (Test-Path (Join-Path $PSScriptRoot '..\bin\gnirehtet.exe'))) {
    Assert-Equal $threw $true '找不到可执行文件时抛异常'
}
Remove-Item Env:PHONE_NETSHARE_EXE
Remove-Item -LiteralPath $tmp

if ($script:Failed -gt 0) { Write-Host "`n$($script:Failed) 项失败" -ForegroundColor Red; exit 1 }
Write-Host "`n全部通过"
