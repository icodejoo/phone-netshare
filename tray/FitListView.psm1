# 自适应列表控件：列宽自动铺满控件宽度、拖动某列时其余列按比例让出空间、永远没有横向滚动条，
# 文字放不下时由系统以省略号截断。用 C# 子类实现，避免在 PowerShell 事件里反复改列宽造成的抖动。

Add-Type -AssemblyName System.Windows.Forms, System.Drawing

# 控件的 C# 源码
$script:Source = @'
using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace NetShare {
    /// <summary>详细视图列表：列宽始终等于控件宽度，无横向滚动条。</summary>
    public class FitListView : ListView {
        /// <summary>横向滚动条标识</summary>
        private const int SB_HORZ = 0;
        /// <summary>窗口消息：重绘、尺寸变化、横向滚动</summary>
        private const int WM_PAINT = 0x000F, WM_SIZE = 0x0005, WM_HSCROLL = 0x0114;

        /// <summary>显示或隐藏窗口滚动条</summary>
        [DllImport("user32.dll")] private static extern bool ShowScrollBar(IntPtr hWnd, int bar, bool show);

        /// <summary>各列占控件宽度的比例，和为 1</summary>
        private double[] ratios;
        /// <summary>程序自己改列宽期间为 true，避免递归处理</summary>
        private bool fitting;

        /// <summary>每列最小宽度（像素）</summary>
        public int MinColumnWidth = 40;

        /// <summary>创建控件：详细视图、整行选中、选中态在失焦后保留、双缓冲</summary>
        public FitListView() {
            View = System.Windows.Forms.View.Details;
            FullRowSelect = true;
            HideSelection = false;
            MultiSelect = false;
            DoubleBuffered = true;
        }

        /// <summary>设置各列初始权重（按列顺序，个数需与列数一致）并立即铺满</summary>
        public void SetWeights(double[] weights) {
            double sum = 0;
            foreach (double w in weights) sum += w;
            ratios = new double[weights.Length];
            for (int i = 0; i < weights.Length; i++) ratios[i] = weights[i] / sum;
            Fit();
        }

        /// <summary>按当前比例把列宽铺满控件宽度；最后一列吃取整余量</summary>
        private void Fit() {
            int n = Columns.Count, total = ClientSize.Width;
            if (ratios == null || ratios.Length != n || total <= 0 || n == 0) return;
            fitting = true;
            try {
                int used = 0;
                for (int i = 0; i < n - 1; i++) {
                    int room = total - used - MinColumnWidth * (n - 1 - i);
                    int w = Math.Max(MinColumnWidth, Math.Min((int)(total * ratios[i]), room));
                    Columns[i].Width = w;
                    used += w;
                }
                Columns[n - 1].Width = Math.Max(1, total - used);
            } finally { fitting = false; }
        }

        /// <summary>控件尺寸（含滚动条出现/消失）变化时重新铺满</summary>
        protected override void OnClientSizeChanged(EventArgs e) {
            base.OnClientSizeChanged(e);
            Fit();
        }

        /// <summary>拖动中：最后一列的边界不可拖；其他列不能宽到让后面的列低于最小宽度</summary>
        protected override void OnColumnWidthChanging(ColumnWidthChangingEventArgs e) {
            base.OnColumnWidthChanging(e);
            if (fitting) return;
            int n = Columns.Count;
            if (e.ColumnIndex >= n - 1) { e.Cancel = true; return; }
            int before = 0;
            for (int j = 0; j < e.ColumnIndex; j++) before += Columns[j].Width;
            int max = ClientSize.Width - before - MinColumnWidth * (n - 1 - e.ColumnIndex);
            e.NewWidth = Math.Max(MinColumnWidth, Math.Min(e.NewWidth, max));
        }

        /// <summary>拖完：后面的列按原比例瓜分剩余宽度，并把当前宽度记为新比例</summary>
        protected override void OnColumnWidthChanged(ColumnWidthChangedEventArgs e) {
            base.OnColumnWidthChanged(e);
            if (fitting) return;
            int n = Columns.Count, total = ClientSize.Width;
            if (n == 0 || total <= 0) return;
            fitting = true;
            try {
                int head = 0;
                for (int j = 0; j <= e.ColumnIndex && j < n; j++) head += Columns[j].Width;
                int tailOld = 0;
                for (int j = e.ColumnIndex + 1; j < n; j++) tailOld += Columns[j].Width;
                int remain = total - head, used = 0;
                for (int j = e.ColumnIndex + 1; j < n; j++) {
                    int w = (j == n - 1) ? remain - used
                        : (int)(remain * (tailOld > 0 ? (double)Columns[j].Width / tailOld : 1.0 / (n - 1 - e.ColumnIndex)));
                    w = Math.Max(MinColumnWidth, w);
                    Columns[j].Width = w;
                    used += w;
                }
                ratios = new double[n];
                for (int j = 0; j < n; j++) ratios[j] = (double)Columns[j].Width / total;
            } finally { fitting = false; }
        }

        /// <summary>每次重绘/尺寸变化后确保横向滚动条是隐藏的</summary>
        protected override void WndProc(ref Message m) {
            base.WndProc(ref m);
            if (IsHandleCreated && (m.Msg == WM_PAINT || m.Msg == WM_SIZE || m.Msg == WM_HSCROLL)) ShowScrollBar(Handle, SB_HORZ, false);
        }
    }
}
'@
if (-not ('NetShare.FitListView' -as [type])) {
    Add-Type -TypeDefinition $script:Source -ReferencedAssemblies (
        [System.Windows.Forms.Form].Assembly.Location, [System.Drawing.Color].Assembly.Location,
        [System.Drawing.Graphics].Assembly.Location, [System.ComponentModel.Component].Assembly.Location,
        [System.Drawing.Point].Assembly.Location, [System.Collections.IEnumerable].Assembly.Location,
        [System.Windows.Forms.Message].Assembly.Location, [System.ComponentModel.CancelEventArgs].Assembly.Location)
}

<#
.SYNOPSIS 创建自适应列表：列宽按权重铺满，拖动时其余列让出空间，无横向滚动条，放不下的文字显示省略号。
.PARAMETER Headers 列标题
.PARAMETER Weights 各列初始宽度权重
.OUTPUTS NetShare.FitListView（Dock=Fill）
.EXAMPLE $lv = New-FitListView @('项目', '值') @(1.5, 6)
#>
function New-FitListView([string[]]$Headers, [double[]]$Weights) {
    $lv = New-Object NetShare.FitListView
    $lv.Dock = 'Fill'
    foreach ($h in $Headers) { [void]$lv.Columns.Add($h, 100) }
    $lv.SetWeights($Weights)
    return $lv
}

Export-ModuleMember -Function New-FitListView
