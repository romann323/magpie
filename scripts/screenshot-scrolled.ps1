param(
    [string]$Exe = "src-tauri\target\release\desktop.exe",
    [string]$Out = "screenshots\magpie-delete-button.png",
    [int]$WaitSeconds = 8
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
public class Win2 {
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
    [DllImport("user32.dll")]
    public static extern IntPtr SetCursorPos(int X, int Y);
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, IntPtr dwExtraInfo);
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    public const uint PW_RENDERFULLCONTENT = 0x2;
    public const uint MOUSEEVENTF_WHEEL = 0x0800;
    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const uint MOUSEEVENTF_LEFTUP = 0x0004;
}
"@ -ReferencedAssemblies "System.Drawing"

$proc = Start-Process -FilePath (Resolve-Path $Exe) -PassThru
Start-Sleep -Seconds $WaitSeconds

$found = @()
$cb = [Win2+EnumWindowsProc] {
    param($h, $lp)
    if (-not [Win2]::IsWindowVisible($h)) { return $true }
    $sb = New-Object System.Text.StringBuilder 256
    [Win2]::GetWindowText($h, $sb, 256) | Out-Null
    $title = $sb.ToString()
    $ownerPid = 0
    [Win2]::GetWindowThreadProcessId($h, [ref]$ownerPid) | Out-Null
    if ($title -eq "Magpie" -and $ownerPid -ne 0) {
        $script:found += [pscustomobject]@{ Hwnd = $h; OwnerPid = $ownerPid }
    }
    return $true
}
[Win2]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found.Count -eq 0) { throw "No Magpie window" }
$hwnd = $found[0].Hwnd

$r = New-Object Win2+Rect
[Win2]::GetWindowRect($hwnd, [ref]$r) | Out-Null
$w = $r.Right - $r.Left
$h = $r.Bottom - $r.Top

# Focus window
[Win2]::ShowWindow($hwnd, 9) | Out-Null
[Win2]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 500

# 1. Click a thumbnail (roughly middle-left area) to open the Details panel
$clickX = $r.Left + 500
$clickY = $r.Top + 300
[Win2]::SetCursorPos($clickX, $clickY) | Out-Null
Start-Sleep -Milliseconds 200
[Win2]::mouse_event([Win2]::MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 50
[Win2]::mouse_event([Win2]::MOUSEEVENTF_LEFTUP, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 500

# 2. Move cursor into details panel (right side) and scroll down
$scrollX = $r.Right - 170
$scrollY = $r.Top + 500
[Win2]::SetCursorPos($scrollX, $scrollY) | Out-Null
Start-Sleep -Milliseconds 100
$scrollAmount = [uint32]([BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]-240), 0))
for ($i = 0; $i -lt 10; $i++) {
    [Win2]::mouse_event([Win2]::MOUSEEVENTF_WHEEL, 0, 0, $scrollAmount, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
}
Start-Sleep -Milliseconds 500

# 3. Screenshot
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [Win2]::PrintWindow($hwnd, $hdc, [Win2]::PW_RENDERFULLCONTENT)
$g.ReleaseHdc($hdc)
if ($ok) {
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host "Saved to $Out"
} else {
    throw "PrintWindow failed"
}
$g.Dispose(); $bmp.Dispose()

try { $proc | Stop-Process -Force -ErrorAction SilentlyContinue } catch {}
Start-Sleep -Seconds 1
Write-Host "Done."
