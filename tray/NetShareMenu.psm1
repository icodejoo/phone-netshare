# 托盘菜单外观与弹出位置：自定义渲染器（在菜单项右侧画绿色勾号）、按光标位置选择弹出方向。

Add-Type -AssemblyName System.Windows.Forms, System.Drawing

# 渲染器与原生调用的 C# 源码
$script:Source = @'
using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace NetShare {
    /// <summary>菜单渲染器：AccessibleDescription 为 "checked" 的项，在文字右侧画绿色勾号。</summary>
    public class MenuRenderer : ToolStripProfessionalRenderer {
        /// <summary>标记已选中项的 AccessibleDescription 取值</summary>
        public const string CheckedMark = "checked";
        /// <summary>勾号颜色</summary>
        public Color CheckColor = Color.FromArgb(22, 163, 74);

        /// <summary>先按默认方式画文字，再在右侧补勾号</summary>
        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e) {
            base.OnRenderItemText(e);
            if (e.Item.AccessibleDescription != CheckedMark) return;
            Graphics g = e.Graphics;
            float k = g.DpiX / 96f;
            SmoothingMode old = g.SmoothingMode;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            float x = e.Item.Width - 19f * k;
            float y = e.Item.Height / 2f;
            using (Pen pen = new Pen(CheckColor, 2f * k)) {
                pen.StartCap = LineCap.Round; pen.EndCap = LineCap.Round; pen.LineJoin = LineJoin.Round;
                g.DrawLines(pen, new PointF[] {
                    new PointF(x, y), new PointF(x + 4f * k, y + 4f * k), new PointF(x + 12f * k, y - 4f * k) });
            }
            g.SmoothingMode = old;
        }
    }

    /// <summary>原生窗口调用</summary>
    public static class Native {
        /// <summary>把窗口置为前台；托盘菜单需要它才能在点击别处时自动收起</summary>
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    }
}
'@
if (-not ('NetShare.MenuRenderer' -as [type])) {
    Add-Type -TypeDefinition $script:Source -ReferencedAssemblies (
        [System.Windows.Forms.Form].Assembly.Location, [System.Drawing.Color].Assembly.Location,
        [System.Drawing.Graphics].Assembly.Location, [System.ComponentModel.Component].Assembly.Location,
        [System.Drawing.Point].Assembly.Location)
}

<# .SYNOPSIS 创建带勾号绘制的菜单渲染器。 #>
function New-NetShareMenuRenderer { New-Object NetShare.MenuRenderer }

<# .SYNOPSIS 勾号标记值：把它赋给菜单项的 AccessibleDescription，该项右侧就会画勾号。 #>
function Get-CheckedMark { [NetShare.MenuRenderer]::CheckedMark }

<#
.SYNOPSIS 在鼠标位置弹出菜单：优先向光标右侧展开，放不下才靠到左侧；任务栏在下方时向上弹，在上方时向下弹。
.PARAMETER Menu 要弹出的菜单
.EXAMPLE Show-MenuAtCursor $menu
#>
function Show-MenuAtCursor([System.Windows.Forms.ContextMenuStrip]$Menu) {
    $pt = [System.Windows.Forms.Control]::MousePosition
    $wa = [System.Windows.Forms.Screen]::FromPoint($pt).WorkingArea
    $dir = if ($pt.Y -gt $wa.Top + $wa.Height / 2) { 'AboveRight' } else { 'BelowRight' }
    [void]$Menu.Handle
    [void][NetShare.Native]::SetForegroundWindow($Menu.Handle)
    $Menu.Show($pt, [System.Windows.Forms.ToolStripDropDownDirection]$dir)
}

Export-ModuleMember -Function New-NetShareMenuRenderer, Get-CheckedMark, Show-MenuAtCursor
