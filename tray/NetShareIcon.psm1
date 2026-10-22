# 托盘图标：运行时用 GDI+ 画一个「手机 + 信号」图标，不依赖任何图片文件。
# 共享中为绿色手机带信号弧，空闲为灰色手机只留一个点。

Add-Type -AssemblyName System.Drawing
# 释放 GetHicon 产生的句柄用
if (-not ('NetShare.NativeIcon' -as [type])) {
    Add-Type -Namespace NetShare -Name NativeIcon -MemberDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool DestroyIcon(System.IntPtr handle);'
}

# 共享中的主色（绿）
$script:ColorActive = '#16A34A'
# 空闲时的主色（灰）
$script:ColorIdle = '#6B7280'
# 作图的逻辑画布边长，所有坐标按它设计，再按目标尺寸缩放
$script:Canvas = 64

<#
.SYNOPSIS 生成圆角矩形路径。
.PARAMETER X,Y,W,H 矩形位置与大小
.PARAMETER R 圆角半径
#>
function New-RoundRectPath([float]$X, [float]$Y, [float]$W, [float]$H, [float]$R) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $R * 2
    $p.AddArc($X, $Y, $d, $d, 180, 90)
    $p.AddArc($X + $W - $d, $Y, $d, $d, 270, 90)
    $p.AddArc($X + $W - $d, $Y + $H - $d, $d, $d, 0, 90)
    $p.AddArc($X, $Y + $H - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

<#
.SYNOPSIS 画出图标位图（透明底）。
.PARAMETER Size 边长（像素）
.PARAMETER Active 是否画成「共享中」样式
.OUTPUTS System.Drawing.Bitmap，调用方负责释放
.EXAMPLE $bmp = New-NetShareBitmap 32 $true
#>
function New-NetShareBitmap([int]$Size, [bool]$Active) {
    $bmp = New-Object System.Drawing.Bitmap $Size, $Size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.ScaleTransform($Size / $script:Canvas, $Size / $script:Canvas)
    $color = [System.Drawing.ColorTranslator]::FromHtml($(if ($Active) { $script:ColorActive } else { $script:ColorIdle }))
    $brush = New-Object System.Drawing.SolidBrush $color
    $white = [System.Drawing.Brushes]::White
    # 机身与屏幕
    $body = New-RoundRectPath 14 3 36 58 8
    $g.FillPath($brush, $body)
    $screen = New-RoundRectPath 19 10 26 40 3
    $g.FillPath($white, $screen)
    # 信号：圆心在屏幕下部，共享时画三道弧，空闲只留圆点
    $cx = 32; $cy = 43
    if ($Active) {
        $pen = New-Object System.Drawing.Pen $color, 3.4
        $pen.StartCap = 'Round'; $pen.EndCap = 'Round'
        foreach ($r in 6, 12, 18) { $g.DrawArc($pen, $cx - $r, $cy - $r, $r * 2, $r * 2, 225, 90) }
        $pen.Dispose()
    }
    $g.FillEllipse($brush, $cx - 3.2, $cy - 3.2, 6.4, 6.4)
    $brush.Dispose(); $body.Dispose(); $screen.Dispose(); $g.Dispose()
    return $bmp
}

<#
.SYNOPSIS 生成托盘用的 Icon 对象。句柄在进程内常驻（只会创建两个：空闲、共享中）。
.PARAMETER Active 是否「共享中」样式
.PARAMETER Size 边长，默认 32
.OUTPUTS System.Drawing.Icon
.EXAMPLE $icon = New-NetShareIcon $false
#>
function New-NetShareIcon([bool]$Active, [int]$Size = 32) {
    $bmp = New-NetShareBitmap $Size $Active
    $icon = [System.Drawing.Icon]::FromHandle($bmp.GetHicon())
    $bmp.Dispose()
    return $icon
}

<#
.SYNOPSIS 把图标存成多尺寸 .ico（PNG 内嵌格式），给桌面快捷方式用。
.PARAMETER Path 输出文件路径
.PARAMETER Active 是否「共享中」样式，默认 $true
.EXAMPLE Save-NetShareIco bin\netshare.ico
#>
function Save-NetShareIco([string]$Path, [bool]$Active = $true) {
    $sizes = 16, 32, 48, 64
    $pngs = foreach ($s in $sizes) {
        $bmp = New-NetShareBitmap $s $Active
        $ms = New-Object System.IO.MemoryStream
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $bmp.Dispose()
        , $ms.ToArray()
    }
    $out = New-Object System.IO.MemoryStream
    $w = New-Object System.IO.BinaryWriter $out
    $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $w.Write([byte]$sizes[$i]); $w.Write([byte]$sizes[$i]); $w.Write([byte]0); $w.Write([byte]0)
        $w.Write([uint16]1); $w.Write([uint16]32)
        $w.Write([uint32]$pngs[$i].Length); $w.Write([uint32]$offset)
        $offset += $pngs[$i].Length
    }
    foreach ($p in $pngs) { $w.Write($p) }
    $w.Flush()
    New-Item -ItemType Directory -Force (Split-Path -Parent $Path) | Out-Null
    [System.IO.File]::WriteAllBytes($Path, $out.ToArray())
}

Export-ModuleMember -Function New-NetShareBitmap, New-NetShareIcon, Save-NetShareIco
