# WeChat Agent start script for Windows PowerShell
#
# Usage:
#   .\start.ps1            Start WeChat developer tools & backend
#   .\start.ps1 -Help      Show help
#
# If PowerShell blocks scripts, run:
#   powershell -ExecutionPolicy Bypass -File .\start.ps1

[CmdletBinding(DefaultParameterSetName = 'Serve')]
param(
    [Parameter(ParameterSetName = 'Help')]  [switch]$Help
)

$ErrorActionPreference = 'Stop'

$Root       = $PSScriptRoot

function Info($m) { Write-Host "[start] $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "[ ok ] $m" -ForegroundColor Green }
function Warn($m) { Write-Host "[warn] $m" -ForegroundColor Yellow }
function Fail($m) { Write-Host "[fail] $m" -ForegroundColor Red }

if ($Help) {
    foreach ($line in (Get-Content $PSCommandPath)) {
        if ($line -match '^#') { Write-Host ($line -replace '^# ?', '') }
        elseif ($line -match '^\s*$') { Write-Host '' }
        else { break }
    }
    exit 0
}

# ── Detect LAN IP and patch config files ──
function Get-LanIp {
    $ip = Get-NetIPAddress -AddressFamily IPv4 |
          Where-Object { $_.IPAddress -match '^(192\.168|10\.|172\.(1[6-9]|2\d|3[01]))\.' -and $_.PrefixOrigin -ne 'WellKnown' } |
          Select-Object -First 1 -ExpandProperty IPAddress
    if (-not $ip) {
        $ip = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -ne '127.0.0.1' } |
               Select-Object -First 1).IPAddress
    }
    return $ip
}

function Patch-File([string]$Path, [string]$Pattern, [string]$Replacement) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $content = [System.IO.File]::ReadAllText($Path, $utf8)
    $newContent = $content -replace $Pattern, $Replacement
    if ($newContent -ne $content) {
        [System.IO.File]::WriteAllText($Path, $newContent, $utf8)
        return $true
    }
    return $false
}

function Patch-Configs([string]$Ip) {
    $base = "http://${Ip}:3000" # 后端端口现在是3000
    $patched = 0

    # miniprogram/utils/store.ts — dev baseUrl
    $storePath = Join-Path $Root 'miniprogram\utils\store.ts'
    if (Test-Path $storePath) {
        if (Patch-File $storePath 'http://[^:]+:3000' $base) { $patched++ }
        if (Patch-File $storePath 'http://[^:]+:8200' $base) { $patched++ }
    }

    # miniprogram/app.js (如果有写死的话)
    $appPath = Join-Path $Root 'miniprogram\app.js'
    if (Test-Path $appPath) {
        if (Patch-File $appPath 'http://[^:]+:3000' $base) { $patched++ }
        if (Patch-File $appPath 'http://[^:]+:8200' $base) { $patched++ }
    }

    # app.ts
    $appTsPath = Join-Path $Root 'miniprogram\app.ts'
    if (Test-Path $appTsPath) {
        if (Patch-File $appTsPath 'http://[^:]+:3000' $base) { $patched++ }
        if (Patch-File $appTsPath 'http://[^:]+:8200' $base) { $patched++ }
    }

    Ok "Patched $patched frontend file(s) -> API URL: $base"
}

function Stop-PortListeners([int[]]$Ports) {
    foreach ($port in $Ports) {
        $conns = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
        foreach ($c in $conns) {
            $procId = $c.OwningProcess
            if ($procId -and $procId -ne 0) {
                $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
                if ($proc) {
                    Warn "Killing process $($proc.Name) (PID $procId) on port $port..."
                    Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
                    Start-Sleep -Milliseconds 200
                }
            }
        }
    }
}

Info 'Auto-detecting LAN IP for WeChat MiniProgram preview...'
$lanIp = Get-LanIp
if ($lanIp) {
    Patch-Configs $lanIp
} else {
    Warn 'Could not detect LAN IP, configs left unchanged.'
}

Info 'Cleaning up old processes on port 3000...'
Stop-PortListeners @(3000)

# --- Open WeChat Developer Tools ---
Info 'Checking WeChat Developer Tools...'
$cliCmd = $null
if (Get-Command "cli.bat" -ErrorAction SilentlyContinue) {
    $cliCmd = "cli.bat"
} elseif (Get-Command "cli.cmd" -ErrorAction SilentlyContinue) {
    $cliCmd = "cli.cmd"
} elseif (Get-Command "cli" -ErrorAction SilentlyContinue) {
    $cliCmd = "cli"
}

if (-not $cliCmd) {
    $defaultPaths = @(
        "C:\Program Files (x86)\Tencent\微信web开发者工具\cli.bat",
        "C:\Program Files\Tencent\微信web开发者工具\cli.bat",
        "D:\Program Files (x86)\Tencent\微信web开发者工具\cli.bat",
        "D:\Program Files\Tencent\微信web开发者工具\cli.bat"
    )
    foreach ($path in $defaultPaths) {
        if (Test-Path $path) {
            $cliCmd = $path
            break
        }
    }
}

if ($cliCmd) {
    Info 'Opening WeChat Developer Tools...'
    try {
        $cliArgs = "open --project `"$Root`""
        Start-Process -FilePath $cliCmd -ArgumentList $cliArgs -NoNewWindow
        Ok 'WeChat Developer Tools launching in background.'
    } catch {
        Warn "Failed to auto-open WeChat Developer Tools: $_"
    }
} else {
    Warn "Missing cli.bat. Please open WeChat Developer Tools manually."
}

# --- Start Backend via start-backend.ps1 ---
$startBackendScript = Join-Path $Root 'start-backend.ps1'
if (Test-Path $startBackendScript) {
    Info "Starting backend server using $startBackendScript..."
    # 传递控制权给 start-backend.ps1
    & $startBackendScript
} else {
    Fail "Could not find $startBackendScript!"
    exit 1
}
