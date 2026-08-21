param(
    [string]$Exe = "src-tauri\target\release\desktop.exe",
    [string]$Out = "screenshots\multiselect.png",
    [int]$StartupWaitSeconds = 12,
    # Where to check that our tag actually appears in the DB.
    [string]$Db = "$env:APPDATA\com.magpie.app\library.db",
    [string]$TestTag = "batchtag-{0}" -f ([DateTime]::UtcNow.ToString("HHmmss"))
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
    [DllImport("user32.dll")]
    public static extern IntPtr SetCursorPos(int x, int y);
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const uint MOUSEEVENTF_LEFTUP = 0x0004;
    public const uint PW_RENDERFULLCONTENT = 0x2;
}
"@ -ReferencedAssemblies "System.Drawing"

function Get-AppWindow {
    $script:found = New-Object System.Collections.ArrayList
    $cb = [Win+EnumWindowsProc] {
        param($h, $lp)
        if (-not [Win]::IsWindowVisible($h)) { return $true }
        $sb = New-Object System.Text.StringBuilder 256
        [Win]::GetWindowText($h, $sb, 256) | Out-Null
        $title = $sb.ToString()
        if ($title -like "*Magpie*") {
            [void]$script:found.Add([pscustomobject]@{ Hwnd = $h; Title = $title })
        }
        return $true
    }
    [Win]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
    return $script:found
}

function Save-WindowScreenshot([IntPtr]$hwnd, [string]$outFile) {
    $r = New-Object Win+Rect
    [Win]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [Win]::PrintWindow($hwnd, $hdc, [Win]::PW_RENDERFULLCONTENT) | Out-Null
    $g.ReleaseHdc($hdc)
    $bmp.Save($outFile, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    return @{ Left = $r.Left; Top = $r.Top; W = $w; H = $h }
}

function Click-At([int]$x, [int]$y, [bool]$withCtrl) {
    if ($withCtrl) { [System.Windows.Forms.SendKeys]::SendWait('^') | Out-Null; Start-Sleep -Milliseconds 20 }
    [Win]::SetCursorPos($x, $y) | Out-Null
    Start-Sleep -Milliseconds 40
    # If we want ctrl+click, hold ctrl via keybd_event. SendKeys can't hold.
    if ($withCtrl) {
        # Use SendInput via SendKeys is not enough. Use a small P/Invoke approach: press
        # Ctrl using keybd_event.
        $kb = Add-Type -PassThru -Namespace 'KbFn' -Name 'Kb' -MemberDefinition @"
[DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
"@
        $VK_CONTROL = [byte]0x11
        $KEYEVENTF_KEYUP = [uint32]2
        $kb::keybd_event($VK_CONTROL, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 30
        [Win]::mouse_event([Win]::MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 30
        [Win]::mouse_event([Win]::MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 30
        $kb::keybd_event($VK_CONTROL, 0, $KEYEVENTF_KEYUP, [UIntPtr]::Zero)
    } else {
        [Win]::mouse_event([Win]::MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 30
        [Win]::mouse_event([Win]::MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 100
}

$exePath = Resolve-Path $Exe
Write-Host "Launching $exePath"
$proc = Start-Process -FilePath $exePath -PassThru
Start-Sleep -Seconds $StartupWaitSeconds

$wins = Get-AppWindow
if ($wins.Count -eq 0) {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    throw "Magpie window not found."
}
$hwnd = $wins[0].Hwnd
Write-Host "Found Magpie window: hwnd=$hwnd title='$($wins[0].Title)'"

[Win]::ShowWindow($hwnd, 9) | Out-Null
[Win]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 500

# Take a baseline screenshot so we can eyeball layout.
$geom = Save-WindowScreenshot $hwnd (Join-Path $outDir "baseline.png")
Write-Host "Window rect: $($geom.W)x$($geom.H) at ($($geom.Left),$($geom.Top))"

# Best-effort: image grid usually starts around x=250, y=120.
# We click at two positions to select two images.
$firstX = $geom.Left + 320
$firstY = $geom.Top + 200
$secondX = $geom.Left + 480
$secondY = $geom.Top + 200

Write-Host "Click 1: ($firstX,$firstY)"
Click-At $firstX $firstY $false
Start-Sleep -Milliseconds 300
Write-Host "Ctrl+Click 2: ($secondX,$secondY)"
Click-At $secondX $secondY $true
Start-Sleep -Milliseconds 500

Save-WindowScreenshot $hwnd (Join-Path $outDir "after-select.png") | Out-Null

# Focus the "Add tags" input by tabbing until we hit it, or click at the
# expected right-panel input location. Right panel starts around x = geom.W - 320.
# The Add tags TagInput placeholder is on the right side.
$inputX = $geom.Left + $geom.W - 200
$inputY = $geom.Top + 260
Click-At $inputX $inputY $false
Start-Sleep -Milliseconds 300

# Type the tag; the input commits on space.
[System.Windows.Forms.SendKeys]::SendWait($TestTag)
Start-Sleep -Milliseconds 200
[System.Windows.Forms.SendKeys]::SendWait(' ')
Start-Sleep -Milliseconds 400

Save-WindowScreenshot $hwnd (Join-Path $outDir "after-type.png") | Out-Null

# Click "Apply tag changes" — the primary button below the inputs. Coarse guess.
$applyX = $geom.Left + $geom.W - 180
$applyY = $geom.Top + 380
Click-At $applyX $applyY $false
Start-Sleep -Seconds 2

Save-WindowScreenshot $hwnd $Out | Out-Null

Write-Host "TestTag = $TestTag"
Write-Host "Screenshots:"
Get-ChildItem $outDir | Select-Object Name, Length | Format-Table

# Give the sidecar-write pipeline a moment to finish.
Start-Sleep -Seconds 2

# Kill the app so we can read the DB safely.
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

# Inspect DB.
if (Test-Path $Db) {
    Write-Host "`nDB check for tag '$TestTag':"
    # Use sqlite3 via .NET SqliteConnection? PowerShell doesn't ship with sqlite CLI.
    # Instead call the tag list via a tiny Rust example.
    Push-Location src-tauri
    $env:MAGPIE_QUERY_TAG = $TestTag
    cargo run -q --example dump_tag_usage 2>&1
    Pop-Location
}
