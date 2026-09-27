<#
Installs Akhsakov Finance for the current user on Windows (x64).

    irm https://raw.githubusercontent.com/Ax-47/akhsakov_finance/main/installer.ps1 | iex

It runs the release's setup (per user, no administrator needed) and adds the
`akhsakov-finance` command:

    akhsakov-finance                open the app
    akhsakov-finance run server     run only the server (--lan for phones)
    akhsakov-finance update         install the latest release
    akhsakov-finance uninstall      remove the app (--purge: and your data)

Options, e.g. with & ([scriptblock]::Create((irm <url>))) -Version v0.2.0
    -Version TAG      install this release instead of the latest one
                      (or set $env:AKHSAKOV_VERSION)
    -From FILE        install a local setup .exe
    -NoModifyPath     don't add the command to your PATH
    -Uninstall        remove the app and keep your data
    -Purge            remove the app and your data
#>
param(
    [string]$Version = $env:AKHSAKOV_VERSION,
    [string]$From = "",
    [switch]$NoModifyPath,
    [switch]$Uninstall,
    [switch]$Purge
)

$ErrorActionPreference = 'Stop'
# Invoke-WebRequest is many times slower with its progress bar on.
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'Ax-47/akhsakov_finance'
$AppName = 'Akhsakov Finance'
$InstallerUrl = "https://raw.githubusercontent.com/$Repo/main/installer.ps1"
# The setup's uninstall entry, named after the bundle identifier.
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\com.akhsakov.finance'
# The folder the app itself uses for its data (src/local_server.rs).
$DataDir = Join-Path $env:LOCALAPPDATA 'akhsakov-finance'
$BinDir = Join-Path $DataDir 'bin'
$DefaultAppDir = Join-Path $env:LOCALAPPDATA 'Programs\AkhsakovFinance'

function Get-AppDir {
    $entry = Get-ItemProperty -Path $UninstallKey -ErrorAction SilentlyContinue
    if ($entry -and $entry.InstallLocation) { return $entry.InstallLocation.TrimEnd('\') }
    return $DefaultAppDir
}

# The `akhsakov-finance` command: a .cmd that runs this script with the
# install's folders filled in. Uninstalling runs its `uninstall`, so the
# installer and the command remove the same things.
function Get-CliScript([string]$AppDir) {
    $quote = { param($s) "'" + ($s -replace "'", "''") + "'" }
    $header = @(
        "# The $AppName command. Written by installer.ps1; ``akhsakov-finance update`` replaces it.",
        "`$AppDir = $(& $quote $AppDir)",
        "`$DataDir = $(& $quote $DataDir)",
        "`$BinDir = $(& $quote $BinDir)",
        "`$InstallerUrl = $(& $quote $InstallerUrl)",
        "`$UninstallKey = $(& $quote $UninstallKey)"
    ) -join "`r`n"
    $body = @'

$ErrorActionPreference = 'Stop'
$AppName = 'Akhsakov Finance'
$App = Join-Path $AppDir 'akhsakov-finance.exe'
$Server = Join-Path $AppDir 'akhsakov-finance-server.exe'

function Show-Usage {
    @"
Usage: akhsakov-finance [command]

  run [app]               open the app (the default)
  run server [options]    run only the server, e.g. for the phone app
      --lan               let phones and computers on your network connect
      --port N            listen on port N (default 8080, which the app uses)
  update [--version TAG]  install the latest release
  uninstall [--purge]     remove the app; --purge also deletes your data
  version                 show the installed version

Your data: $DataDir
"@
}

function Stop-WithError([string]$Message) {
    [Console]::Error.WriteLine("akhsakov-finance: $Message")
    exit 1
}

function Test-Port([int]$Port) {
    $client = New-Object Net.Sockets.TcpClient
    try {
        $attempt = $client.BeginConnect('127.0.0.1', $Port, $null, $null)
        return ($attempt.AsyncWaitHandle.WaitOne(300) -and $client.Connected)
    } catch {
        return $false
    } finally {
        $client.Close()
    }
}

# The server's sign-in state, or $null while it isn't answering.
function Get-ServerStatus([int]$Port) {
    try {
        return Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$Port/api/auth/status" `
            -Body '{}' -ContentType 'application/json' -TimeoutSec 2
    } catch {
        return $null
    }
}

function Start-App {
    if (-not (Test-Path $App)) { Stop-WithError "$App is missing. Reinstall with: akhsakov-finance update" }
    # The app starts its own server.
    Start-Process -FilePath $App
}

function Start-Server([string[]]$Options) {
    $address = '127.0.0.1'
    $port = 8080
    for ($i = 0; $i -lt $Options.Count; $i++) {
        switch -Regex ($Options[$i]) {
            '^--lan$' { $address = '0.0.0.0' }
            '^--port$' {
                $i++
                if ($i -ge $Options.Count) { Stop-WithError '--port needs a number' }
                $port = $Options[$i]
            }
            '^--port=' { $port = $Options[$i].Substring(7) }
            '^(-h|--help)$' { Show-Usage; exit 0 }
            default { Stop-WithError "unknown option for run server: $($Options[$i])" }
        }
    }
    $number = 0
    if (-not [int]::TryParse([string]$port, [ref]$number) -or $number -lt 1 -or $number -gt 65535) {
        Stop-WithError '--port needs a number from 1 to 65535'
    }
    $port = $number
    if (-not (Test-Path $Server)) { Stop-WithError "$Server is missing. Reinstall with: akhsakov-finance update" }
    if (Get-ServerStatus $port) {
        Stop-WithError ("a server is already running on port $port, probably the app's own. Close the app, run this`n" +
            "again, then open the app: it uses this server while it runs.")
    }
    if (Test-Port $port) { Stop-WithError "port $port is taken by another program. Pick another with --port." }

    $public = Join-Path $DataDir 'public'
    New-Item -ItemType Directory -Force -Path $public | Out-Null
    if (-not $env:AKHSAKOV_DB) { $env:AKHSAKOV_DB = Join-Path $DataDir 'akhsakov_finance.db' }
    $env:IP = $address
    $env:PORT = "$port"
    $env:DIOXUS_PUBLIC_PATH = $public
    Write-Host "Starting the $AppName server..."
    $process = Start-Process -FilePath $Server -NoNewWindow -PassThru
    try {
        # The first start creates the database, so give it a while.
        $status = $null
        for ($i = 0; $i -lt 150 -and -not $status; $i++) {
            if ($process.HasExited) { Stop-WithError "the server didn't start (see the messages above)." }
            Start-Sleep -Milliseconds 200
            $status = Get-ServerStatus $port
        }
        if (-not $status) { Stop-WithError "the server didn't start (see the messages above)." }
        Write-Host ''
        Write-Host "OK. The server is running. Data: $env:AKHSAKOV_DB"
        if ($address -eq '0.0.0.0') {
            Write-Host '  In the phone app, enter one of these addresses:'
            $found = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' }
            if (-not $found) { Write-Host "    http://<this computer's IP address>:$port" }
            foreach ($ip in $found) { Write-Host "    http://$($ip.IPAddress):$port" }
            if ($status.needs_setup) {
                Write-Host '  Anyone on your network can open your data until you create an account'
                Write-Host '  (Settings > Security in the app).'
            }
        } else {
            Write-Host "  Only this computer can connect: http://127.0.0.1:$port"
            Write-Host '  For the phone app, run: akhsakov-finance run server --lan'
        }
        Write-Host '  Press Ctrl+C to stop.'
        $process.WaitForExit()
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
}

function Remove-FromPath {
    $path = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $path) { return }
    $kept = @($path -split ';' | Where-Object { $_ -and $_.TrimEnd('\') -ne $BinDir })
    [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
}

function Remove-App([string[]]$Options) {
    $purge = $false
    foreach ($option in $Options) {
        if ($option -eq '--purge') { $purge = $true } else { Stop-WithError "unknown option for uninstall: $option" }
    }
    Get-Process -Name 'akhsakov-finance', 'akhsakov-finance-server' -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    $uninstaller = Join-Path $AppDir 'uninstall.exe'
    if (Test-Path $uninstaller) {
        # _?= runs it in place, so it finishes before this goes on.
        Start-Process -FilePath $uninstaller -ArgumentList '/S', "_?=$AppDir" -Wait
    }
    Remove-Item -Recurse -Force -Path $AppDir -ErrorAction SilentlyContinue
    Remove-Item -Path $UninstallKey -Recurse -Force -ErrorAction SilentlyContinue
    Remove-FromPath
    Remove-Item -Force -Path (Join-Path $DataDir 'VERSION') -ErrorAction SilentlyContinue
    # Safe while the .cmd that started this runs: it has already read the
    # line that ends it.
    Remove-Item -Recurse -Force -Path $BinDir -ErrorAction SilentlyContinue
    if ($purge) {
        Remove-Item -Recurse -Force -Path $DataDir -ErrorAction SilentlyContinue
        Write-Host "OK. $AppName and its data were removed."
    } else {
        Write-Host "OK. $AppName was removed."
        Write-Host "  Your data is still in $DataDir"
        Write-Host "  (to delete it too: & ([scriptblock]::Create((irm $InstallerUrl))) -Purge)"
    }
}

$command = if ($args.Count -gt 0) { $args[0] } else { 'run' }
$rest = @(if ($args.Count -gt 1) { $args[1..($args.Count - 1)] })
switch ($command) {
    'run' {
        $what = if ($rest.Count -gt 0) { $rest[0] } else { 'app' }
        $options = @(if ($rest.Count -gt 1) { $rest[1..($rest.Count - 1)] })
        switch ($what) {
            'app' { Start-App }
            'server' { Start-Server $options }
            default { Stop-WithError 'run what? Use: akhsakov-finance run [app|server]' }
        }
    }
    'update' {
        if ($rest.Count -ge 2 -and $rest[0] -eq '--version') { $env:AKHSAKOV_VERSION = $rest[1] }
        elseif ($rest.Count -gt 0) { Stop-WithError 'use: akhsakov-finance update [--version TAG]' }
        $url = if ($env:AKHSAKOV_INSTALLER_URL) { $env:AKHSAKOV_INSTALLER_URL } else { $InstallerUrl }
        & ([scriptblock]::Create((Invoke-RestMethod -Uri $url)))
    }
    'uninstall' { Remove-App $rest }
    { $_ -in 'version', '--version' } {
        $file = Join-Path $DataDir 'VERSION'
        if (Test-Path $file) { Get-Content $file } else { Stop-WithError 'no version recorded' }
    }
    { $_ -in 'help', '-h', '--help' } { Show-Usage }
    default { Show-Usage; exit 1 }
}
'@
    return $header + "`r`n" + $body
}

function Write-Cli([string]$AppDir) {
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    Set-Content -Path (Join-Path $BinDir 'akhsakov-finance.ps1') -Value (Get-CliScript $AppDir) -Encoding UTF8
    # One line, so cmd never reads this file again after PowerShell returns
    # (`update` and `uninstall` replace or delete it).
    $cmd = "@echo off`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"%~dp0akhsakov-finance.ps1`" %* & exit /b`r`n"
    Set-Content -Path (Join-Path $BinDir 'akhsakov-finance.cmd') -Value $cmd -Encoding ASCII -NoNewline
}

# Adds the command's folder to the user's PATH; returns whether it changed.
function Add-ToPath {
    $path = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @(if ($path) { $path -split ';' | Where-Object { $_ } })
    if ($entries | Where-Object { $_.TrimEnd('\') -eq $BinDir }) { return $false }
    [Environment]::SetEnvironmentVariable('Path', (($entries + $BinDir) -join ';'), 'User')
    $env:Path = "$env:Path;$BinDir"
    return $true
}

function Install-AkhsakovFinance {
    if ($Uninstall -or $Purge) {
        $cli = Join-Path ([IO.Path]::GetTempPath()) "akhsakov-finance-uninstall-$PID.ps1"
        Set-Content -Path $cli -Value (Get-CliScript (Get-AppDir)) -Encoding UTF8
        $options = @('uninstall')
        if ($Purge) { $options += '--purge' }
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $cli @options
        Remove-Item -Force -Path $cli -ErrorAction SilentlyContinue
        return
    }

    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }
    if ($arch -eq 'ARM64') {
        Write-Warning 'Releases are built for x64; Windows on ARM runs them through emulation.'
    } elseif ($arch -ne 'AMD64') {
        throw "Releases are built for x64 Windows; this PC is $arch."
    }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) "akhsakov-finance-install-$PID"
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    try {
        $setup = Join-Path $tmp 'setup.exe'
        if ($From) {
            if (-not (Test-Path $From)) { throw "No such file: $From" }
            Copy-Item -Path $From -Destination $setup
            $tag = 'local'
        } else {
            if ($Version) {
                $api = "https://api.github.com/repos/$Repo/releases/tags/$Version"
                Write-Host "Looking up release $Version of $AppName..."
            } else {
                $api = "https://api.github.com/repos/$Repo/releases/latest"
                Write-Host "Looking up the latest release of $AppName..."
            }
            try {
                $release = Invoke-RestMethod -Uri $api -Headers @{ Accept = 'application/vnd.github+json' }
            } catch {
                throw "Couldn't read $api (no such release, or GitHub is unreachable)."
            }
            $tag = $release.tag_name
            $asset = $release.assets | Where-Object { $_.name -like '*windows*x86_64*setup.exe' } | Select-Object -First 1
            if (-not $asset) { throw "Release $tag has no Windows build." }
            Write-Host "Downloading $($asset.name)..."
            Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $setup -UseBasicParsing
            $sums = $release.assets | Where-Object { $_.name -eq 'SHA256SUMS' } | Select-Object -First 1
            if ($sums) {
                $list = (Invoke-WebRequest -Uri $sums.browser_download_url -UseBasicParsing).Content
                if ($list -is [byte[]]) { $list = [Text.Encoding]::UTF8.GetString($list) }
                $line = $list -split "`n" | Where-Object { $_ -match "^([0-9a-f]{64})\s+\*?$([regex]::Escape($asset.name))\s*$" } | Select-Object -First 1
                if (-not $line) { throw "$($asset.name) isn't listed in the release's SHA256SUMS." }
                $expected = ($line -split '\s+')[0]
                $actual = (Get-FileHash -Algorithm SHA256 -Path $setup).Hash.ToLower()
                if ($actual -ne $expected) { throw "$($asset.name) doesn't match its checksum; the download may be damaged. Try again." }
            }
        }

        # Files in use can't be replaced, so close a running copy first.
        $running = Get-Process -Name 'akhsakov-finance', 'akhsakov-finance-server' -ErrorAction SilentlyContinue
        if ($running) {
            Write-Host "Closing the running $AppName to update it..."
            $running | Stop-Process -Force
            Start-Sleep -Milliseconds 500
        }
        Write-Host 'Installing...'
        $result = Start-Process -FilePath $setup -ArgumentList '/S' -Wait -PassThru
        if ($result.ExitCode -ne 0) { throw "Setup failed (exit code $($result.ExitCode))." }
        $appDir = Get-AppDir
        foreach ($file in 'akhsakov-finance.exe', 'akhsakov-finance-server.exe') {
            if (-not (Test-Path (Join-Path $appDir $file))) { throw "Setup didn't install $file into $appDir." }
        }
    } finally {
        Remove-Item -Recurse -Force -Path $tmp -ErrorAction SilentlyContinue
    }

    New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
    $versionFile = Join-Path $DataDir 'VERSION'
    $previous = if (Test-Path $versionFile) { (Get-Content $versionFile -Raw).Trim() } else { '' }
    Set-Content -Path $versionFile -Value $tag -Encoding ASCII
    Write-Cli $appDir
    $pathChanged = $false
    if (-not $NoModifyPath) { $pathChanged = Add-ToPath }

    Write-Host ''
    if ($previous -and $previous -ne $tag) {
        Write-Host "OK. $AppName updated from $previous to $tag."
    } else {
        Write-Host "OK. $AppName $tag installed."
    }
    Write-Host '  Open it from the Start menu, or run: akhsakov-finance'
    Write-Host '  For the phone app: akhsakov-finance run server --lan'
    Write-Host "  Your data: $DataDir"
    if ($pathChanged) { Write-Host '  Open a new terminal to use the akhsakov-finance command.' }
}

Install-AkhsakovFinance
