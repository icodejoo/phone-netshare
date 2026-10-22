# 用原 PowerShell 版的 NetShareIcon.psm1 画托盘图标，导出为 32x32 原始 RGBA（Rust 版 include_bytes 嵌入）。
# 用法：pwsh -File scripts\export-icons.ps1
# 输出：app\icons\idle.rgba、app\icons\active.rgba（各 32*32*4 = 4096 字节，非预乘 RGBA）
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot '..\tray\NetShareIcon.psm1') -Force

# 托盘图标边长，与原版 New-NetShareIcon 的默认值一致
$Size = 32
$OutDir = Join-Path $PSScriptRoot '..\app\icons'
New-Item -ItemType Directory -Force $OutDir | Out-Null

foreach ($state in @(@{ Name = 'idle'; Active = $false }, @{ Name = 'active'; Active = $true })) {
    $bmp = New-NetShareBitmap $Size $state.Active
    $bytes = New-Object byte[] ($Size * $Size * 4)
    $i = 0
    for ($y = 0; $y -lt $Size; $y++) {
        for ($x = 0; $x -lt $Size; $x++) {
            $c = $bmp.GetPixel($x, $y)
            $bytes[$i++] = $c.R; $bytes[$i++] = $c.G; $bytes[$i++] = $c.B; $bytes[$i++] = $c.A
        }
    }
    $bmp.Dispose()
    $path = Join-Path (Resolve-Path $OutDir) "$($state.Name).rgba"
    [System.IO.File]::WriteAllBytes($path, $bytes)
    Write-Host "$path ($($bytes.Length) 字节)"
}
