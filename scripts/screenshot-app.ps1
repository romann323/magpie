param(
    [string]$Exe = "src-tauri\target\release\picorg.exe",
    [string]$Out = "screenshots\picorg-launch.png",
    [int]$WaitSeconds = 10
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$outDir = Split-Path -Parent $Out
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Force -Path $outDir | Out-Null }

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Drawing;
public class Win {
    [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out Rect r);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")]
    public static extern int GetWindowThreadProcessId(IntPtr hWnd, out int lpdwProcessId);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", CharSet=CharSet.Auto)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }

    public const uint PW_CLIENTONLY = 0x1;
    public const uint PW_RENDERFULLCONTENT = 0x2;
}
"@ -ReferencedAssemblies "System.Drawing"

$exePath = Resolve-Path $Exe
Write-Host "Launching $exePath"
$proc = Start-Process -FilePath $exePath -PassThru
$targetPid = $proc.Id
Write-Host "PID: $targetPid"

Start-Sleep -Seconds $WaitSeconds

if ($proc.HasExited) {
    throw "App exited before screenshot (exit code $($proc.ExitCode))"
}

$found = @()
$cb = [Win+EnumWindowsProc] {
    param($h, $lp)
    if (-not [Win]::IsWindowVisible($h)) { return $true }
    $sb = New-Object System.Text.StringBuilder 256
    [Win]::GetWindowText($h, $sb, 256) | Out-Null
    $title = $sb.ToString()
    $ownerPid = 0
    [Win]::GetWindowThreadProcessId($h, [ref]$ownerPid) | Out-Null
    if ($title -like "*PicOrg*" -and $ownerPid -ne 0) {
        $script:found += [pscustomobject]@{ Hwnd = $h; Title = $title; OwnerPid = $ownerPid }
    }
    return $true
}
[Win]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null

Write-Host "Windows found: $($found.Count)"
foreach ($w in $found) { Write-Host "  hwnd=$($w.Hwnd) pid=$($w.OwnerPid) title='$($w.Title)'" }

if ($found.Count -eq 0) {
    throw "No PicOrg window found"
}

$hwnd = $found[0].Hwnd
$r = New-Object Win+Rect
if (-not [Win]::GetWindowRect($hwnd, [ref]$r)) {
    throw "GetWindowRect failed"
}
$w = $r.Right - $r.Left
$h = $r.Bottom - $r.Top
Write-Host "Window rect: ${w}x${h} at ($($r.Left),$($r.Top))"

$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# PW_RENDERFULLCONTENT is required for Chromium-based content (WebView2)
$ok = [Win]::PrintWindow($hwnd, $hdc, [Win]::PW_RENDERFULLCONTENT)
$g.ReleaseHdc($hdc)

if ($ok) {
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host "Saved via PrintWindow to $Out"
} else {
    Write-Host "PrintWindow failed, falling back to screen copy after activation"
    [Win]::ShowWindow($hwnd, 9) | Out-Null
    [Win]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Seconds 2
    $g2 = [System.Drawing.Graphics]::FromImage($bmp)
    $g2.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g2.Dispose()
    Write-Host "Saved via screen copy to $Out"
}

$g.Dispose(); $bmp.Dispose()

try { $proc | Stop-Process -Force -ErrorAction SilentlyContinue } catch { }
Start-Sleep -Seconds 1
Write-Host "Done."
