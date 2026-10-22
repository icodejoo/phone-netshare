# NetShare：手机反向共享电脑网络的核心逻辑（基于 gnirehtet），不含任何界面代码，便于单测。
# 依赖：PATH 里有 adb；gnirehtet.exe 见 Get-GnirehtetExe 的查找顺序。

# gnirehtet 中继服务的固定监听端口
$script:RelayPort = 31416
# 手机端 App 包名
$script:AppPkg = 'com.genymobile.gnirehtet'
# 手机端隧道网卡名，存在即表示共享已生效
$script:TunName = 'tun0'
# 本模块启动的中继进程；不是本模块启动的不会去关
$script:Relay = $null

<#
.SYNOPSIS 定位 gnirehtet.exe。
.DESCRIPTION 查找顺序：环境变量 PHONE_NETSHARE_EXE → 仓库根目录 bin\gnirehtet.exe → PATH。
.OUTPUTS 可执行文件的完整路径；找不到时抛异常。
.EXAMPLE Get-GnirehtetExe
#>
function Get-GnirehtetExe {
    $root = Split-Path -Parent $PSScriptRoot
    $candidates = @($env:PHONE_NETSHARE_EXE, (Join-Path $root 'bin\gnirehtet.exe'))
    foreach ($c in $candidates) { if ($c -and (Test-Path -LiteralPath $c)) { return (Resolve-Path -LiteralPath $c).Path } }
    $cmd = Get-Command gnirehtet.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw '找不到 gnirehtet.exe：请先运行 scripts\install.ps1，或设置环境变量 PHONE_NETSHARE_EXE'
}

<#
.SYNOPSIS 解析 `adb devices -l` 的输出，只保留已授权在线的设备。
.PARAMETER Lines 命令输出的行（含首行 "List of devices attached"）
.OUTPUTS 每台一个对象：Serial（序列号）、Name（型号，缺省取序列号）。
.EXAMPLE ConvertFrom-AdbDevices (adb devices -l)
#>
function ConvertFrom-AdbDevices([string[]]$Lines) {
    foreach ($line in $Lines) {
        if ($line -match '^(\S+)\s+device\b') {
            $serial = $Matches[1]
            $name = if ($line -match 'model:(\S+)') { $Matches[1] } else { $serial }
            [pscustomobject]@{ Serial = $serial; Name = $name }
        }
    }
}

<#
.SYNOPSIS 列出当前连接且已授权的手机。
.OUTPUTS 同 ConvertFrom-AdbDevices。
#>
function Get-Phones { ConvertFrom-AdbDevices @(adb devices -l) }

# 详情里要读的系统属性：显示名 → getprop 键
$script:InfoProps = [ordered]@{
    model        = 'ro.product.model'
    brand        = 'ro.product.brand'
    manufacturer = 'ro.product.manufacturer'
    marketname   = 'ro.product.marketname'
    device       = 'ro.product.device'
    release      = 'ro.build.version.release'
    sdk          = 'ro.build.version.sdk'
    abi          = 'ro.product.cpu.abi'
}
# 空值在界面上的占位
$script:Empty = '—'

<#
.SYNOPSIS 追加一行日志到 %TEMP%\phone-netshare.log（界面事件里的异常会被 WinForms 吞掉，靠它排查）。
.PARAMETER Message 日志内容
#>
function Write-NetShareLog([string]$Message) {
    Add-Content -LiteralPath (Join-Path ([IO.Path]::GetTempPath()) 'phone-netshare.log') -Value ('{0:HH:mm:ss} {1}' -f (Get-Date), $Message)
}

<#
.SYNOPSIS 解析 `键=值` 形式的多行文本（只按第一个 = 拆分）。
.PARAMETER Lines 文本行
.OUTPUTS 有序哈希表：键 → 值（已去首尾空白）
#>
function ConvertFrom-KeyValueLines([string[]]$Lines) {
    $kv = [ordered]@{}
    foreach ($line in $Lines) {
        $i = $line.IndexOf('=')
        if ($i -gt 0) { $kv[$line.Substring(0, $i).Trim()] = $line.Substring($i + 1).Trim() }
    }
    return $kv
}

<#
.SYNOPSIS 把读到的系统属性整理成界面要显示的「项目 → 值」列表。
.PARAMETER Kv 属性键值（键见 InfoProps 的名字，另含 device_name）
.PARAMETER Serial 手机序列号
.OUTPUTS 有序哈希表：名称、型号、品牌、制造商、设备代号、系统、CPU 架构、序列号；取不到的显示「—」
#>
function ConvertTo-PhoneInfo($Kv, [string]$Serial) {
    $get = { param($k) $v = $Kv[$k]; if ($v -and $v -ne 'null') { $v } else { $script:Empty } }
    $name = @('device_name', 'marketname', 'model') | ForEach-Object { & $get $_ } | Where-Object { $_ -ne $script:Empty } | Select-Object -First 1
    $release = & $get 'release'
    $sdk = & $get 'sdk'
    $os = if ($release -eq $script:Empty) { $script:Empty } else { "Android $release (SDK $sdk)" }
    return [ordered]@{
        '名称'    = if ($name) { $name } else { $script:Empty }
        '型号'    = & $get 'model'
        '品牌'    = & $get 'brand'
        '制造商'  = & $get 'manufacturer'
        '设备代号' = & $get 'device'
        '系统'    = $os
        'CPU 架构' = & $get 'abi'
        '序列号'  = $Serial
    }
}

<#
.SYNOPSIS 读取一台手机的基本信息（设备名、型号、品牌、系统版本等）。
.PARAMETER Serial 手机序列号
.OUTPUTS 同 ConvertTo-PhoneInfo
#>
function Get-PhoneInfo([string]$Serial) {
    $parts = foreach ($e in $script:InfoProps.GetEnumerator()) { "echo $($e.Key)=`$(getprop $($e.Value))" }
    $cmd = ($parts -join '; ') + '; echo device_name=$(settings get global device_name)'
    ConvertTo-PhoneInfo (ConvertFrom-KeyValueLines @(adb -s $Serial shell $cmd)) $Serial
}

<#
.SYNOPSIS 给每台手机生成菜单上显示的名字：型号；多台同型号时追加序列号末 4 位区分。
.PARAMETER Phones Get-Phones 的结果
.OUTPUTS 与输入同序的字符串数组
#>
function Get-PhoneLabels($Phones) {
    $phones = @($Phones)
    foreach ($p in $phones) {
        $same = @($phones | Where-Object { $_.Name -eq $p.Name }).Count
        if ($same -gt 1) { '{0} ({1})' -f $p.Name, $p.Serial.Substring([Math]::Max(0, $p.Serial.Length - 4)) } else { $p.Name }
    }
}

<#
.SYNOPSIS 判断某台手机当前是否在共享电脑网络（手机上有隧道网卡）。
.PARAMETER Serial 手机序列号
.OUTPUTS 布尔值
#>
function Test-Sharing([string]$Serial) {
    $out = adb -s $Serial shell "ip -brief addr show $($script:TunName) 2>/dev/null"
    return [bool]($out -match $script:TunName)
}

<#
.SYNOPSIS 中继服务是否在监听。
.OUTPUTS 布尔值
#>
function Test-RelayRunning {
    return [bool](Get-NetTCPConnection -LocalPort $script:RelayPort -State Listen -ErrorAction SilentlyContinue)
}

<# .SYNOPSIS 确保中继服务在跑，没有就隐藏窗口启动一个。 #>
function Start-Relay {
    if (Test-RelayRunning) { return }
    $script:Relay = Start-Process -FilePath (Get-GnirehtetExe) -ArgumentList 'relay' -WindowStyle Hidden -PassThru
    Start-Sleep -Milliseconds 800
}

<# .SYNOPSIS 没有任何手机在共享时，关掉本模块启动的中继。 #>
function Stop-RelayIfIdle {
    if (-not $script:Relay -or $script:Relay.HasExited) { return }
    foreach ($p in @(Get-Phones)) { if (Test-Sharing $p.Serial) { return } }
    Stop-Process -Id $script:Relay.Id -Force -ErrorAction SilentlyContinue
    $script:Relay = $null
}

<#
.SYNOPSIS 对一台手机启用共享：确保中继在跑、必要时安装手机端 App、启动客户端。
.PARAMETER Serial 手机序列号
.EXAMPLE Enable-Phone 'ABC123'
#>
function Enable-Phone([string]$Serial) {
    $exe = Get-GnirehtetExe
    Start-Relay
    if (-not (adb -s $Serial shell pm list packages $script:AppPkg)) { & $exe install $Serial *> $null }
    & $exe start $Serial *> $null
}

<#
.SYNOPSIS 对一台手机停止共享；没人用了顺带关中继。
.PARAMETER Serial 手机序列号
#>
function Disable-Phone([string]$Serial) {
    & (Get-GnirehtetExe) stop $Serial *> $null
    Stop-RelayIfIdle
}

<#
.SYNOPSIS 校验文件的 sha256。
.PARAMETER Path 文件路径
.PARAMETER Expected 期望的 sha256（十六进制，不分大小写）
.OUTPUTS 一致返回 $true
#>
function Test-FileSha256([string]$Path, [string]$Expected) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq $Expected.Trim()
}

Export-ModuleMember -Function Write-NetShareLog, Get-GnirehtetExe, ConvertFrom-AdbDevices, ConvertFrom-KeyValueLines, ConvertTo-PhoneInfo, Get-PhoneInfo, Get-PhoneLabels, Get-Phones, Test-Sharing, Test-RelayRunning, Start-Relay, Stop-RelayIfIdle, Enable-Phone, Disable-Phone, Test-FileSha256
