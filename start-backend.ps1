# 星推胡同后端服务快捷启动脚本
# 用法: 双击 start-backend.bat 或在终端执行: powershell -ExecutionPolicy Bypass -File start-backend.ps1

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$BackendDir = Join-Path $ProjectRoot "backend"

Write-Host "================================" -ForegroundColor Cyan
Write-Host "  StarTuiHuTong Backend Launcher" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

# 1. 检查 .env 文件
$EnvFile = Join-Path $BackendDir ".env"
if (-not (Test-Path $EnvFile)) {
    Write-Host "[init] Creating .env from .env.example ..." -ForegroundColor Yellow
    $ExampleFile = Join-Path $BackendDir ".env.example"
    if (Test-Path $ExampleFile) {
        Copy-Item $ExampleFile $EnvFile
        $content = Get-Content $EnvFile -Raw
        $content = $content -replace 'ADMIN_PASSWORD=replace-with-a-strong-password', 'ADMIN_PASSWORD=admin123'
        Set-Content $EnvFile $content
        Write-Host "[init] .env created (ADMIN_PASSWORD=admin123)" -ForegroundColor Green
    } else {
        Write-Host "[ERROR] .env.example not found!" -ForegroundColor Red
        exit 1
    }
}

# 2. 创建 data 目录
$DataDir = Join-Path $BackendDir "data"
if (-not (Test-Path $DataDir)) {
    Write-Host "[init] Creating data directory ..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $DataDir | Out-Null
}

# 3. 选择可用的 Rust 工具链
# 直接扫描文件系统, 不调用 rustup (避免触发 rust-toolchain.toml 的自动安装)
Write-Host "[check] Rust toolchain ..." -ForegroundColor Yellow

$toolchainDir = Join-Path $env:USERPROFILE ".rustup\toolchains"
if (-not (Test-Path $toolchainDir)) {
    Write-Host "[ERROR] rustup toolchains directory not found: $toolchainDir" -ForegroundColor Red
    Write-Host "       Please install Rust: https://rustup.rs" -ForegroundColor Red
    exit 1
}

# 按优先级排序的候选版本
$candidates = @('1.97.0','1.95.0','1.91.1','1.89.0','1.85.0','stable')
$selectedToolchain = $null

foreach ($tc in $candidates) {
    $tcFull = if ($tc -eq 'stable') { 'stable-x86_64-pc-windows-msvc' } else { "$tc-x86_64-pc-windows-msvc" }
    $rustcPath = Join-Path $toolchainDir "$tcFull\bin\rustc.exe"
    if (-not (Test-Path $rustcPath)) { continue }

    # 该工具链存在且 rustc 可执行, 选中它
    $selectedToolchain = $tcFull
    $versionStr = & $rustcPath --version 2>&1
    Write-Host "[check] Toolchain: $tcFull" -ForegroundColor Green
    Write-Host "[check] $versionStr" -ForegroundColor Green
    break
}

if (-not $selectedToolchain) {
    Write-Host "[ERROR] No usable Rust toolchain found!" -ForegroundColor Red
    Write-Host "       Installed toolchains:" -ForegroundColor Yellow
    Get-ChildItem $toolchainDir -Directory | ForEach-Object { Write-Host "         $($_.Name)" -ForegroundColor DarkGray }
    Write-Host "       Please run: rustup toolchain install 1.97.0 --profile minimal" -ForegroundColor Red
    exit 1
}

# 设置环境变量, 覆盖 rust-toolchain.toml 的指定
$env:RUSTUP_TOOLCHAIN = $selectedToolchain

# 4. 启动后端服务
Write-Host ""
Write-Host "[start] Compiling and starting backend (first build may take a few minutes) ..." -ForegroundColor Yellow
Write-Host "[start] API:   http://127.0.0.1:3000" -ForegroundColor Green
Write-Host "[start] Admin: http://127.0.0.1:3000/admin/" -ForegroundColor Green
Write-Host "[start] Press Ctrl+C to stop" -ForegroundColor DarkGray
Write-Host ""

Set-Location $BackendDir
cargo run
