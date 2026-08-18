<#
.SYNOPSIS
    Build the two mdBook sites and generate a downloadable PDF for each.

.DESCRIPTION
    1. Copies docs/shared/{theme.css,pdf-link.js} into each book folder
       so mdBook picks them up as `additional-css` / `additional-js`.
    2. Runs `mdbook build` for docs/user-manual and docs/developer.
    3. Uses headless Microsoft Edge to render each book's print.html
       to a PDF at docs/picorg-user-manual.pdf and
       docs/picorg-developer-guide.pdf.

.PARAMETER Serve
    After building, serve docs/ on http://localhost:8000 so you can
    inspect the result.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File docs\build.ps1
#>
param(
    [switch]$Serve
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$docsRoot = Join-Path $repoRoot 'docs'
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

if (-not (Get-Command mdbook -ErrorAction SilentlyContinue)) {
    throw "mdbook is not installed. Run: cargo install mdbook --version 0.4.40 --locked"
}

# Step 0: sync shared assets into each book directory.
# mdBook's additional-css / additional-js do not copy files from
# outside the book root, so we mirror them into each book folder
# first. Each book.toml references them by bare filename.
$sharedDir = Join-Path $docsRoot 'shared'
foreach ($book in @('user-manual', 'developer')) {
    $target = Join-Path $docsRoot $book
    Copy-Item -Path (Join-Path $sharedDir 'theme.css')   -Destination $target -Force
    Copy-Item -Path (Join-Path $sharedDir 'pdf-link.js') -Destination $target -Force
}

# Step 1: build both books.
$env:RUST_LOG = 'warn'
Write-Host "[1/3] Building user-manual"
Push-Location (Join-Path $docsRoot 'user-manual')
mdbook build 2>&1 | Where-Object { $_ -notmatch 'DEBUG' } | Select-Object -Last 3 | ForEach-Object { Write-Host "    $_" }
Pop-Location

Write-Host "[2/3] Building developer guide"
Push-Location (Join-Path $docsRoot 'developer')
mdbook build 2>&1 | Where-Object { $_ -notmatch 'DEBUG' } | Select-Object -Last 3 | ForEach-Object { Write-Host "    $_" }
Pop-Location

# Step 2: locate an Edge/Chrome binary that supports --headless=new.
function Get-BrowserBinary {
    $candidates = @(
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:LocalAppData\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe"
    )
    foreach ($c in $candidates) {
        if ($c -and (Test-Path $c)) { return $c }
    }
    return $null
}

$browser = Get-BrowserBinary
if (-not $browser) {
    Write-Warning "No Edge/Chrome found. The HTML sites are still built; PDF generation skipped."
    return
}
Write-Host "[3/3] Rendering PDFs via $browser"

function Convert-BookToPdf {
    param(
        [string]$BookRoot,
        [string]$PdfOut,
        [string]$Title
    )
    $printHtml = Join-Path $BookRoot 'book\print.html'
    if (-not (Test-Path $printHtml)) {
        throw "print.html missing at $printHtml"
    }
    # Delete any stale PDF so we can tell if the fresh render succeeded.
    if (Test-Path $PdfOut) { Remove-Item $PdfOut -Force }

    $uri = 'file:///' + ($printHtml -replace '\\', '/')
    Write-Host "    $Title  ->  $PdfOut"

    # Use a per-render user-data-dir so parallel/repeated invocations
    # do not step on a running Edge profile.
    $userDataDir = Join-Path $env:TEMP ("picorg-pdf-" + [guid]::NewGuid().ToString('N'))

    $argList = @(
        '--headless=new',
        '--disable-gpu',
        '--no-sandbox',
        '--no-pdf-header-footer',
        '--print-to-pdf-no-header',
        "--user-data-dir=$userDataDir",
        "--print-to-pdf=$PdfOut",
        $uri
    )

    # Start-Process handles stderr chatter gracefully. Edge exits ~0 once
    # the print job is written. We give it up to 60s.
    $stdoutLog = Join-Path $env:TEMP ("picorg-pdf-out-" + [guid]::NewGuid().ToString('N') + '.log')
    $stderrLog = Join-Path $env:TEMP ("picorg-pdf-err-" + [guid]::NewGuid().ToString('N') + '.log')
    $proc = Start-Process -FilePath $browser -ArgumentList $argList `
        -RedirectStandardOutput $stdoutLog -RedirectStandardError $stderrLog `
        -NoNewWindow -PassThru
    if (-not $proc.WaitForExit(60000)) {
        try { $proc.Kill() } catch { }
        throw "Edge render timed out after 60s for $Title"
    }
    Remove-Item $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
    Remove-Item $userDataDir -Recurse -Force -ErrorAction SilentlyContinue

    if (-not (Test-Path $PdfOut)) {
        throw "Failed to generate PDF at $PdfOut (Edge exit code $($proc.ExitCode))"
    }
    $sizeKb = [math]::Round((Get-Item $PdfOut).Length / 1KB, 1)
    Write-Host "        ok ($sizeKb KB, exit=$($proc.ExitCode))"
}

Convert-BookToPdf `
    -BookRoot (Join-Path $docsRoot 'user-manual') `
    -PdfOut  (Join-Path $docsRoot 'picorg-user-manual.pdf') `
    -Title   'PicOrg User Manual'

Convert-BookToPdf `
    -BookRoot (Join-Path $docsRoot 'developer') `
    -PdfOut  (Join-Path $docsRoot 'picorg-developer-guide.pdf') `
    -Title   'PicOrg Developer Guide'

Write-Host ""
Write-Host "Done. Open docs\index.html to view."

if ($Serve) {
    Write-Host ""
    Write-Host "Serving on http://localhost:8000 (Ctrl+C to stop)"
    Push-Location $docsRoot
    try {
        if (Get-Command python -ErrorAction SilentlyContinue) {
            python -m http.server 8000
        } elseif (Get-Command py -ErrorAction SilentlyContinue) {
            py -m http.server 8000
        } else {
            Write-Warning "No python found. Open docs\index.html directly in your browser."
        }
    } finally {
        Pop-Location
    }
}
