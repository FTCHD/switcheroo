# Switcheroo installer for Windows (PowerShell 5.1 or newer).
#
#   irm https://raw.githubusercontent.com/ftchd/switcheroo/main/install.ps1 | iex
#
# Downloads the release binary, checks it against the release's SHA256SUMS, installs it under
# your user profile, adds that folder to your PATH, and clears the mark-of-the-web so
# SmartScreen does not object when you run it.
#
# Environment:
#   SWITCHEROO_VERSION      tag to install (default: latest release)
#   SWITCHEROO_INSTALL_DIR  folder for switcheroo.exe (default: %LOCALAPPDATA%\Programs\switcheroo)
#   SWITCHEROO_DRY_RUN=1    print what would happen and exit

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'ftchd/switcheroo'
$version = if ($env:SWITCHEROO_VERSION) { $env:SWITCHEROO_VERSION } else { 'latest' }

$arch = $env:PROCESSOR_ARCHITECTURE
if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }
switch ($arch) {
    'AMD64' { $asset = 'switcheroo-windows-x86_64.exe' }
    'ARM64' { $asset = 'switcheroo-windows-aarch64.exe' }
    default { throw "install.ps1: no prebuilt binary for Windows/$arch; build from source: https://github.com/$repo#install" }
}

$base = if ($version -eq 'latest') {
    "https://github.com/$repo/releases/latest/download"
} else {
    "https://github.com/$repo/releases/download/$version"
}

$dir = if ($env:SWITCHEROO_INSTALL_DIR) {
    $env:SWITCHEROO_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\switcheroo'
}
$target = Join-Path $dir 'switcheroo.exe'

if ($env:SWITCHEROO_DRY_RUN -eq '1') {
    Write-Host "would download $base/$asset and $base/SHA256SUMS"
    Write-Host "would install to $target"
    return
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("switcheroo-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "Downloading $asset ($version)..."
    Invoke-WebRequest -Uri "$base/$asset" -OutFile (Join-Path $tmp 'switcheroo.exe') -UseBasicParsing
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS') -UseBasicParsing

    $line = Get-Content (Join-Path $tmp 'SHA256SUMS') | Where-Object { $_ -match "\s$([regex]::Escape($asset))$" } | Select-Object -First 1
    if (-not $line) { throw "install.ps1: no checksum for $asset in SHA256SUMS" }
    $expected = ($line -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp 'switcheroo.exe')).Hash.ToLowerInvariant()
    if ($expected -ne $actual) { throw "install.ps1: checksum mismatch for $asset (expected $expected, got $actual)" }

    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Move-Item -Force (Join-Path $tmp 'switcheroo.exe') $target
    # Clears the Zone.Identifier stream (mark-of-the-web) if the download added one.
    Unblock-File -Path $target -ErrorAction SilentlyContinue
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $dir)) {
    [Environment]::SetEnvironmentVariable('Path', (($userPath, $dir) -join ';').Trim(';'), 'User')
    $env:Path = "$env:Path;$dir"
    Write-Host "Added $dir to your PATH (open a new terminal for other windows to see it)."
}

Write-Host ("Installed " + (& $target --version) + " to $target")
Write-Host 'Run `switcheroo doctor` to see what it found.'
