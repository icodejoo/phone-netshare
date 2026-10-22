# 托盘界面：左键/右键托盘图标，按手机启用或停止「手机用电脑网络」，支持多台手机；「手机列表…」打开详情面板。
# 用法：pwsh -File phone-netshare-tray.ps1
#   -SelfTest        对所有已连手机跑一遍 启用→检查→停止 后退出
#   -Snapshot <目录> 把托盘菜单和面板截图存到该目录后退出（验收界面用）
param([switch]$SelfTest, [string]$Snapshot)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$ErrorActionPreference = 'Stop'
try { [System.Windows.Forms.Application]::SetHighDpiMode('SystemAware') | Out-Null } catch { }
[System.Windows.Forms.Application]::EnableVisualStyles()
foreach ($m in 'NetShare', 'NetShareIcon', 'NetShareMenu', 'PhonePanel') { Import-Module (Join-Path $PSScriptRoot "$m.psm1") -Force }

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

# 单实例：已有一个托盘在跑就直接退出（截图模式不受限）
if (-not $Snapshot) {
    $mutex = New-Object System.Threading.Mutex($false, 'Local\phone-netshare-tray')
    if (-not $mutex.WaitOne(0)) { exit }
}

# 空闲/共享中两枚图标（句柄进程内常驻）
$script:IconIdle = New-NetShareIcon $false
$script:IconActive = New-NetShareIcon $true
# 托盘图标
$script:Ni = $null
# 右键菜单
$script:Menu = New-Object System.Windows.Forms.ContextMenuStrip
# 状态刷新计时器
$script:Timer = New-Object System.Windows.Forms.Timer
# 平时的状态刷新间隔（毫秒）与操作后的快速复查间隔
$script:IdleMs = 10000
$script:KickMs = 2500
# 已共享手机的文字/勾号颜色（绿）与「退出」的文字颜色（红）
$script:CheckColor = [System.Drawing.Color]::FromArgb(22, 163, 74)
$script:QuitColor = [System.Drawing.Color]::FromArgb(220, 38, 38)
# 手机名后补多少个不换行空格，给勾号预留宽度（随字体和 DPI 自动缩放）
$script:CheckReserveChars = 7

<# .SYNOPSIS 按当前共享情况更新托盘图标与悬停提示。 #>
function Update-TrayState {
    $phones = @(Get-Phones)
    $n = @($phones | Where-Object { Test-Sharing $_.Serial }).Count
    $script:Ni.Icon = if ($n -gt 0) { $script:IconActive } else { $script:IconIdle }
    $script:Ni.Text = if ($n -gt 0) { "手机网络：$n 台共享中" } else { '手机网络：未共享' }
}

<# .SYNOPSIS 操作之后尽快复查一次状态（隧道建立有延迟），之后恢复平时间隔。 #>
function Request-Refresh {
    $script:Timer.Interval = $script:KickMs
    $script:Timer.Stop(); $script:Timer.Start()
}

<# .SYNOPSIS 托盘气泡提示。 #>
function Show-Tip([string]$Text) { $script:Ni.ShowBalloonTip(2000, '手机网络', $Text, 'Info') }

<# .SYNOPSIS 点某台手机：当前在共享则停止，否则启用。 #>
function Switch-Phone([string]$Serial, [string]$Name) {
    if (Test-Sharing $Serial) { Disable-Phone $Serial; Show-Tip "$Name 已停止共享" }
    else { Enable-Phone $Serial; Show-Tip "$Name 已使用电脑网络" }
    Update-TrayState; Request-Refresh
}

<# .SYNOPSIS 打开手机列表面板，并让面板内的操作同步托盘图标。 #>
function Open-Panel { Show-PhonePanel -Icon $script:IconActive -OnChanged { Update-TrayState; Request-Refresh } }

<# .SYNOPSIS 菜单即将弹出时，按当前连着的手机重建菜单。 #>
function Build-Menu {
    $menu = $script:Menu
    $menu.Items.Clear()
    $phones = @(Get-Phones)
    $labels = @(Get-PhoneLabels $phones)
    $panel = $menu.Items.Add('手机列表…')
    $panel.Font = New-Object System.Drawing.Font($menu.Font, 'Bold')
    $panel.Add_Click({ Open-Panel })
    [void]$menu.Items.Add('-')
    if ($phones.Count -eq 0) {
        $none = $menu.Items.Add('没有检测到手机')
        $none.Enabled = $false
    }
    # 勾号占位：每台手机的名字后面补同样数量的不换行空格，撑出右侧空位，勾号画在这块空位里
    $reserve = [string][char]0xA0 * $script:CheckReserveChars
    for ($i = 0; $i -lt $phones.Count; $i++) {
        $p = $phones[$i]
        $on = Test-Sharing $p.Serial
        $item = $menu.Items.Add($labels[$i] + $reserve)
        $item.Tag = [pscustomobject]@{ Serial = $p.Serial; Name = $labels[$i] }
        # 已共享：文字变绿，右侧由渲染器画勾号
        if ($on) { $item.ForeColor = $script:CheckColor; $item.AccessibleDescription = Get-CheckedMark }
        $item.Add_Click({ param($s, $e) Switch-Phone $s.Tag.Serial $s.Tag.Name })
    }
    [void]$menu.Items.Add('-')
    $all = $menu.Items.Add('全部启用')
    $all.Enabled = $phones.Count -gt 0
    $all.Add_Click({ foreach ($p in @(Get-Phones)) { Enable-Phone $p.Serial }; Show-Tip '已对所有手机启用'; Update-TrayState; Request-Refresh })
    $stop = $menu.Items.Add('全部停止')
    $stop.Add_Click({ foreach ($p in @(Get-Phones)) { Disable-Phone $p.Serial }; Show-Tip '已全部停止'; Update-TrayState; Request-Refresh })
    [void]$menu.Items.Add('-')
    $quit = $menu.Items.Add('退出')
    $quit.ForeColor = $script:QuitColor
    $quit.Add_Click({
            foreach ($p in @(Get-Phones)) { Disable-Phone $p.Serial }
            $script:Ni.Visible = $false
            [System.Windows.Forms.Application]::Exit()
        })
}

# 菜单外观：去掉左侧图标/勾选栏，宽度只随文字
$script:Menu.ShowImageMargin = $false
$script:Menu.ShowCheckMargin = $false
$script:Menu.Font = New-Object System.Drawing.Font('Segoe UI', 9)
$script:Menu.Renderer = New-NetShareMenuRenderer

$script:Ni = New-Object System.Windows.Forms.NotifyIcon
$script:Ni.Icon = $script:IconIdle
$script:Ni.Text = '手机网络：未共享'
# 左右键都由自己弹菜单（可控制弹出方向）；双击打开面板
$script:Ni.Add_MouseClick({
        param($s, $e)
        if ($e.Button -eq 'Left' -or $e.Button -eq 'Right') { Build-Menu; Show-MenuAtCursor $script:Menu }
    })
$script:Ni.Add_DoubleClick({ Open-Panel })
$script:Ni.Visible = $true

$script:Timer.Interval = $script:IdleMs
if (-not $Snapshot) {
    $script:Timer.Add_Tick({
            $script:Timer.Interval = $script:IdleMs
            Update-TrayState
        })
}

# 截图模式：弹出菜单与面板各截一张图，存完退出
if ($Snapshot) {
    New-Item -ItemType Directory -Force $Snapshot | Out-Null
    $step = 0
    $script:Timer.Interval = 900
    $script:Timer.Add_Tick({
            $step++
            switch ($step) {
                1 { Build-Menu; $script:Menu.Show(120, 120) }
                2 {
                    $b = $script:Menu.Bounds
                    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
                    [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
                    $bmp.Save((Join-Path $Snapshot 'menu.png')); $bmp.Dispose()
                    $script:Menu.Close(); Open-Panel
                }
                3 { Save-PanelSnapshot (Join-Path $Snapshot 'panel.png'); $script:Ni.Visible = $false; [System.Windows.Forms.Application]::Exit() }
            }
        })
}
$script:Timer.Start()
Update-TrayState
[System.Windows.Forms.Application]::Run()
