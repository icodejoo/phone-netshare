# 手机列表面板：查看已连手机、连接（共享）状态与基本信息，可一键复制名字、型号、序列号等。
# 依赖 NetShare.psm1；界面为 WinForms，需在 STA 线程且有消息循环时使用（托盘进程已满足）。

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Import-Module (Join-Path $PSScriptRoot 'NetShare.psm1')
Import-Module (Join-Path $PSScriptRoot 'FitListView.psm1')

# 面板窗体；null 表示未打开
$script:Form = $null
# 手机列表控件
$script:Lv = $null
# 详情列表控件
$script:Dv = $null
# 底部状态文字
$script:Status = $null
# 「启用/停止」按钮
$script:BtnToggle = $null
# 手机基本信息缓存：序列号 → 信息（这些信息不会变，只读一次）
$script:InfoCache = @{}
# 面板内操作后的回调（让托盘同步图标）
$script:OnChanged = $null
# 状态文字
$script:TextOn = '共享中'
$script:TextOff = '未共享'
# 启用/停止后等隧道状态落定再刷新的时间（毫秒）
$script:SettleMs = 1800

<# .SYNOPSIS 写底部状态栏。 #>
function Set-PanelStatus([string]$Text) { if ($script:Status) { $script:Status.Text = $Text } }

<# .SYNOPSIS 取（必要时读取并缓存）一台手机的基本信息。 #>
function Get-CachedInfo([string]$Serial) {
    if (-not $script:InfoCache.ContainsKey($Serial)) { $script:InfoCache[$Serial] = Get-PhoneInfo $Serial }
    return $script:InfoCache[$Serial]
}

<# .SYNOPSIS 当前在手机列表里选中的那一行的数据；没选返回 $null。 #>
function Get-SelectedRow {
    if ($script:Lv -and $script:Lv.SelectedItems.Count -gt 0) { return $script:Lv.SelectedItems[0].Tag }
}

<# .SYNOPSIS 复制文本到剪贴板，并在状态栏提示。 #>
function Copy-Text([string]$Text, [string]$What) {
    if (-not $Text) { return }
    [System.Windows.Forms.Clipboard]::SetText($Text)
    Set-PanelStatus "已复制$What：$Text"
}

<# .SYNOPSIS 选中手机的全部信息，每行「项目：值」。 #>
function Get-AllInfoText($Row) {
    $lines = @("连接状态：$(if ($Row.Sharing) { $script:TextOn } else { $script:TextOff })")
    foreach ($e in $Row.Info.GetEnumerator()) { $lines += '{0}：{1}' -f $e.Key, $e.Value }
    return $lines -join "`r`n"
}

<# .SYNOPSIS 往详情列表追加一行「项目 / 值」。 #>
function Add-DetailRow([string]$Key, [string]$Value) {
    $item = New-Object System.Windows.Forms.ListViewItem
    $item.Text = $Key
    [void]$item.SubItems.Add($Value)
    [void]$script:Dv.Items.Add($item)
}

<# .SYNOPSIS 按选中手机刷新下方详情列表。 #>
function Update-Detail {
    $script:Dv.Items.Clear()
    $row = Get-SelectedRow
    if (-not $row) { return }
    $state = if ($row.Sharing) { $script:TextOn } else { $script:TextOff }
    Add-DetailRow '连接状态' $state
    foreach ($e in $row.Info.GetEnumerator()) { Add-DetailRow ([string]$e.Key) ([string]$e.Value) }
    $script:BtnToggle.Text = if ($row.Sharing) { '停止共享' } else { '启用共享' }
    $script:BtnToggle.Enabled = $true
}

<# .SYNOPSIS 重新扫描手机并刷新列表（保留当前选中项）。 #>
function Update-List {
    $keep = (Get-SelectedRow).Serial
    $phones = @(Get-Phones)
    $script:Lv.BeginUpdate()
    $script:Lv.Items.Clear()
    foreach ($p in $phones) {
        $info = Get-CachedInfo $p.Serial
        $on = Test-Sharing $p.Serial
        $row = [pscustomobject]@{ Serial = $p.Serial; Sharing = $on; Info = $info }
        $item = New-Object System.Windows.Forms.ListViewItem
        $item.Text = [string]$info['名称']
        $item.UseItemStyleForSubItems = $false
        foreach ($t in $info['型号'], $p.Serial, $(if ($on) { $script:TextOn } else { $script:TextOff })) { [void]$item.SubItems.Add([string]$t) }
        if ($on) { $item.SubItems[3].ForeColor = [System.Drawing.Color]::FromArgb(22, 163, 74) }
        $item.Tag = $row
        [void]$script:Lv.Items.Add($item)
        if ($p.Serial -eq $keep) { $item.Selected = $true }
    }
    if ($script:Lv.Items.Count -gt 0 -and $script:Lv.SelectedItems.Count -eq 0) { $script:Lv.Items[0].Selected = $true }
    $script:Lv.EndUpdate()
    Update-Detail
    if ($script:Lv.Items.Count -eq 0) {
        $script:BtnToggle.Enabled = $false
        Set-PanelStatus '没有检测到手机：请接 USB 并开启 USB 调试'
    }
}

<# .SYNOPSIS 对选中手机切换共享状态，稍后自动再刷新一次（隧道建立有延迟）。 #>
function Switch-Selected {
    $row = Get-SelectedRow
    if (-not $row) { return }
    $script:Form.Cursor = 'WaitCursor'
    try {
        if ($row.Sharing) { Disable-Phone $row.Serial } else { Enable-Phone $row.Serial }
        Start-Sleep -Milliseconds $script:SettleMs
    }
    finally { $script:Form.Cursor = 'Default' }
    Update-List
    Set-PanelStatus '操作已完成'
    if ($script:OnChanged) { & $script:OnChanged }
}

<# .SYNOPSIS 创建带若干菜单项的右键菜单；Items 是 文字→动作 的有序列表。 #>
function New-CopyMenu($Items) {
    $m = New-Object System.Windows.Forms.ContextMenuStrip
    foreach ($e in $Items.GetEnumerator()) {
        $mi = $m.Items.Add($e.Key)
        $mi.Tag = $e.Value
        $mi.Add_Click({ param($s, $a) & $s.Tag })
    }
    return $m
}

<# .SYNOPSIS 创建一个按钮。 #>
function New-Button([string]$Text, [scriptblock]$OnClick) {
    $b = New-Object System.Windows.Forms.Button
    $b.Text = $Text; $b.AutoSize = $true; $b.Padding = New-Object System.Windows.Forms.Padding 6, 1, 6, 1
    $b.Add_Click($OnClick)
    return $b
}

<# .SYNOPSIS 构建面板窗体与全部控件。 #>
function New-PanelForm($Icon) {
    $f = New-Object System.Windows.Forms.Form
    $f.Text = '手机网络'
    $f.Font = New-Object System.Drawing.Font('Segoe UI', 9)
    $f.StartPosition = 'CenterScreen'
    $f.ClientSize = New-Object System.Drawing.Size 760, 480
    $f.MinimumSize = New-Object System.Drawing.Size 640, 420
    if ($Icon) { $f.Icon = $Icon }

    $layout = New-Object System.Windows.Forms.TableLayoutPanel
    $layout.Dock = 'Fill'; $layout.ColumnCount = 1; $layout.RowCount = 5
    $layout.Padding = New-Object System.Windows.Forms.Padding 10
    foreach ($s in @(
            (New-Object System.Windows.Forms.RowStyle 'Percent', 38),
            (New-Object System.Windows.Forms.RowStyle 'Absolute', 26),
            (New-Object System.Windows.Forms.RowStyle 'Percent', 62),
            (New-Object System.Windows.Forms.RowStyle 'Absolute', 40),
            (New-Object System.Windows.Forms.RowStyle 'Absolute', 24))) { [void]$layout.RowStyles.Add($s) }

    $script:Lv = New-FitListView @('手机', '型号', '序列号', '连接状态') @(3, 2.5, 3.5, 1.5)
    $script:Dv = New-FitListView @('项目', '值') @(1.5, 6)
    $caption = New-Object System.Windows.Forms.Label
    $caption.Text = '详细信息（双击一行复制其值）'; $caption.Dock = 'Fill'; $caption.TextAlign = 'BottomLeft'
    $script:Status = New-Object System.Windows.Forms.Label
    $script:Status.Dock = 'Fill'; $script:Status.TextAlign = 'MiddleLeft'; $script:Status.ForeColor = [System.Drawing.Color]::DimGray

    $script:BtnToggle = New-Button '启用共享' { Switch-Selected }
    $buttons = New-Object System.Windows.Forms.FlowLayoutPanel
    $buttons.Dock = 'Fill'; $buttons.FlowDirection = 'LeftToRight'
    foreach ($b in @(
            (New-Button '刷新' { Update-List; Set-PanelStatus '已刷新' }),
            $script:BtnToggle,
            (New-Button '复制全部信息' { $r = Get-SelectedRow; if ($r) { Copy-Text (Get-AllInfoText $r) '全部信息' } }),
            (New-Button '复制所选值' { if ($script:Dv.SelectedItems.Count -gt 0) { Copy-Text $script:Dv.SelectedItems[0].SubItems[1].Text '' } }))) { [void]$buttons.Controls.Add($b) }

    # 右键菜单：手机列表复制名称/型号/序列号；详情复制值/整行
    $script:Lv.ContextMenuStrip = New-CopyMenu ([ordered]@{
            '复制名称'   = { $r = Get-SelectedRow; if ($r) { Copy-Text $r.Info['名称'] '名称' } }
            '复制型号'   = { $r = Get-SelectedRow; if ($r) { Copy-Text $r.Info['型号'] '型号' } }
            '复制序列号' = { $r = Get-SelectedRow; if ($r) { Copy-Text $r.Serial '序列号' } }
            '复制全部信息' = { $r = Get-SelectedRow; if ($r) { Copy-Text (Get-AllInfoText $r) '全部信息' } }
        })
    $script:Dv.ContextMenuStrip = New-CopyMenu ([ordered]@{
            '复制值'   = { if ($script:Dv.SelectedItems.Count -gt 0) { Copy-Text $script:Dv.SelectedItems[0].SubItems[1].Text '' } }
            '复制整行' = { if ($script:Dv.SelectedItems.Count -gt 0) { $i = $script:Dv.SelectedItems[0]; Copy-Text ('{0}：{1}' -f $i.Text, $i.SubItems[1].Text) '' } }
        })
    $script:Dv.Add_DoubleClick({ if ($script:Dv.SelectedItems.Count -gt 0) { Copy-Text $script:Dv.SelectedItems[0].SubItems[1].Text '' } })
    $script:Lv.Add_SelectedIndexChanged({ try { Update-Detail } catch { Write-NetShareLog "Update-Detail: $_" } })

    foreach ($c in $script:Lv, $caption, $script:Dv, $buttons, $script:Status) { [void]$layout.Controls.Add($c) }
    $f.Controls.Add($layout)

    $f.Add_FormClosed({ $script:Form = $null })
    return $f
}

<#
.SYNOPSIS 打开（或把已打开的置顶）手机列表面板。
.PARAMETER Icon 窗口图标，可空
.PARAMETER OnChanged 在面板里启用/停止共享后的回调，可空；托盘用它同步图标
.EXAMPLE Show-PhonePanel -Icon $icon -OnChanged { Update-TrayState }
#>
function Show-PhonePanel($Icon = $null, [scriptblock]$OnChanged = $null) {
    $script:OnChanged = $OnChanged
    if ($script:Form -and -not $script:Form.IsDisposed) {
        Update-List
        $script:Form.Show(); $script:Form.Activate(); $script:Form.BringToFront()
        return
    }
    $script:Form = New-PanelForm $Icon
    $script:Form.Show()
    Update-List
}

<#
.SYNOPSIS 把面板窗口截图存成 png（开发/验收用）；面板必须已打开。
.PARAMETER Path 输出文件
#>
function Save-PanelSnapshot([string]$Path) {
    if (-not $script:Form) { return }
    $bmp = New-Object System.Drawing.Bitmap $script:Form.Width, $script:Form.Height
    $script:Form.DrawToBitmap($bmp, (New-Object System.Drawing.Rectangle 0, 0, $script:Form.Width, $script:Form.Height))
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

Export-ModuleMember -Function Show-PhonePanel, Save-PanelSnapshot
