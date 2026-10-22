# 托盘界面：左键/右键托盘图标，按手机启用或停止「手机用电脑网络」，支持多台手机。
# 用法：pwsh -File phone-netshare-tray.ps1 ；加 -SelfTest 则对所有已连手机跑一遍 启用→检查→停止 后退出。
param([switch]$SelfTest)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'NetShare.psm1') -Force

# 自测：对所有手机启用 → 检查 → 停止，输出结果后退出
if ($SelfTest) {
    foreach ($p in @(Get-Phones)) {
        Enable-Phone $p.Serial; Start-Sleep 3
        $up = Test-Sharing $p.Serial
        Disable-Phone $p.Serial; Start-Sleep 2
        '{0} [{1}] 启用后隧道={2} 停止后隧道={3} 中继在跑={4}' -f $p.Name, $p.Serial, $up, (Test-Sharing $p.Serial), (Test-RelayRunning)
    }
    exit
}

# 单实例：已有一个托盘在跑就直接退出
$mutex = New-Object System.Threading.Mutex($false, 'Local\phone-netshare-tray')
if (-not $mutex.WaitOne(0)) { exit }

# 托盘图标（脚本级，事件回调里要用）
$script:Ni = $null
# 右键菜单
$script:Menu = New-Object System.Windows.Forms.ContextMenuStrip

<# .SYNOPSIS 托盘气泡提示。 #>
function Show-Tip([string]$Text) { $script:Ni.ShowBalloonTip(2000, '手机网络', $Text, 'Info') }

<# .SYNOPSIS 点某台手机：当前在共享则停止，否则启用。 #>
function Switch-Phone([string]$Serial, [string]$Name) {
    if (Test-Sharing $Serial) { Disable-Phone $Serial; Show-Tip "$Name 已停止共享" }
    else { Enable-Phone $Serial; Show-Tip "$Name 已使用电脑网络" }
}

<# .SYNOPSIS 菜单即将弹出时，按当前连着的手机重建菜单。 #>
function Build-Menu {
    $menu = $script:Menu
    $menu.Items.Clear()
    $phones = @(Get-Phones)
    if ($phones.Count -eq 0) {
        $none = $menu.Items.Add('（没有检测到手机，请接 USB 并开调试）')
        $none.Enabled = $false
    }
    foreach ($p in $phones) {
        $mark = if (Test-Sharing $p.Serial) { '✓ ' } else { '    ' }
        $item = $menu.Items.Add("$mark$($p.Name)  [$($p.Serial)]")
        $item.Tag = $p
        $item.Add_Click({ param($s, $e) Switch-Phone $s.Tag.Serial $s.Tag.Name })
    }
    [void]$menu.Items.Add('-')
    $all = $menu.Items.Add('全部启用')
    $all.Enabled = $phones.Count -gt 0
    $all.Add_Click({ foreach ($p in @(Get-Phones)) { Enable-Phone $p.Serial }; Show-Tip '已对所有手机启用' })
    $stop = $menu.Items.Add('全部停止')
    $stop.Add_Click({ foreach ($p in @(Get-Phones)) { Disable-Phone $p.Serial }; Show-Tip '已全部停止' })
    [void]$menu.Items.Add('-')
    $quit = $menu.Items.Add('退出')
    $quit.Add_Click({
        foreach ($p in @(Get-Phones)) { Disable-Phone $p.Serial }
        $script:Ni.Visible = $false
        [System.Windows.Forms.Application]::Exit()
    })
}

$script:Menu.Add_Opening({ Build-Menu })
$script:Ni = New-Object System.Windows.Forms.NotifyIcon
$script:Ni.Icon = [System.Drawing.Icon]::ExtractAssociatedIcon((Get-GnirehtetExe))
$script:Ni.Text = '手机网络（gnirehtet）'
$script:Ni.ContextMenuStrip = $script:Menu
# 左键也弹菜单（系统默认只有右键）
$script:Ni.Add_MouseClick({
    param($s, $e)
    if ($e.Button -eq 'Left') {
        [System.Windows.Forms.NotifyIcon].GetMethod('ShowContextMenu', [Reflection.BindingFlags]'NonPublic,Instance').Invoke($script:Ni, $null)
    }
})
$script:Ni.Visible = $true
[System.Windows.Forms.Application]::Run()
