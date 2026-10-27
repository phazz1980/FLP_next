# Release build of FLP_next and assembly of the distribution folder and archive.
# Usage: build.cmd (double-click) or `powershell -File scripts\build-dist.ps1 [-NoZip]`.
param([switch]$NoZip)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist\FLP_next'
$zip = Join-Path $root 'dist\FLP_next.zip'
$bins = 'flp-editor.exe', 'flp-mcp.exe', 'flp-web.exe', 'flp.exe'

# The editor running from dist is closed before the build and started again afterwards.
$script:reopen = $false

function Start-Editor {
    if (-not $script:reopen) { return }
    $script:reopen = $false
    Write-Host 'Запускаю редактор снова.' -ForegroundColor Cyan
    Start-Process (Join-Path $dist 'flp-editor.exe')
}

function Fail($msg) {
    Write-Host "ОШИБКА: $msg" -ForegroundColor Red
    Start-Editor
    exit 1
}

function Get-DistProcesses($names) {
    Get-Process -Name $names -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($dist, [StringComparison]::OrdinalIgnoreCase) }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Fail 'cargo не найден. Установите Rust: https://rustup.rs (toolchain MSVC).'
}

# Close the editor politely: with unsaved changes it asks to save, so wait for the answer.
$editors = @(Get-DistProcesses 'flp-editor')
if ($editors.Count -gt 0) {
    Write-Host 'Закрываю редактор (если есть несохранённые изменения — ответьте в его окне)...' -ForegroundColor Yellow
    $script:reopen = $true
    $editors | ForEach-Object { [void]$_.CloseMainWindow() }
    $deadline = [DateTime]::Now.AddMinutes(3)
    while ((Get-DistProcesses 'flp-editor') -and [DateTime]::Now -lt $deadline) {
        Start-Sleep -Milliseconds 300
    }
    if (Get-DistProcesses 'flp-editor') { Fail 'редактор не закрылся за 3 минуты — сборка отменена.' }
}

# A running exe (e.g. flp-mcp started by Claude) cannot be overwritten but can be renamed:
# move locked binaries aside so cargo can write new ones; remove leftovers when free.
$release = Join-Path $root 'target\release'
Get-ChildItem $release -Filter '*.exe.old*' -ErrorAction SilentlyContinue |
    ForEach-Object { try { Remove-Item $_.FullName -Force -ErrorAction Stop } catch {} }
foreach ($bin in $bins) {
    $path = Join-Path $release $bin
    if (-not (Test-Path $path)) { continue }
    try {
        [System.IO.File]::Open($path, 'Open', 'ReadWrite', 'None').Close()
    } catch {
        $old = "$path.old-$([DateTime]::Now.Ticks)"
        Move-Item $path $old -Force
        Write-Host "$bin запущен — старая копия отодвинута в $(Split-Path -Leaf $old)" -ForegroundColor Yellow
    }
}

Write-Host '== Сборка (cargo build --release) ==' -ForegroundColor Cyan
Push-Location $root
try {
    & cargo build --release --workspace
    if ($LASTEXITCODE -ne 0) { Fail 'сборка не удалась, см. вывод cargo выше.' }
} finally {
    Pop-Location
}

$running = Get-DistProcesses ($bins | ForEach-Object { $_ -replace '\.exe$' })
if ($running) {
    # Helpers (flp-mcp, flp-web, flp) hold no user data: stop them so the files can be replaced.
    Write-Host "Останавливаю запущенные из dist: $(($running.Name | Sort-Object -Unique) -join ', ')" -ForegroundColor Yellow
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    $deadline = [DateTime]::Now.AddSeconds(10)
    while ((Get-DistProcesses ($bins | ForEach-Object { $_ -replace '\.exe$' })) -and [DateTime]::Now -lt $deadline) {
        Start-Sleep -Milliseconds 200
    }
    $left = @(Get-DistProcesses ($bins | ForEach-Object { $_ -replace '\.exe$' }))
    if ($left.Count -gt 0) {
        Fail "не удалось остановить: $(($left.Name | Sort-Object -Unique) -join ', ')."
    }
}

Write-Host '== Сборка папки dist\FLP_next ==' -ForegroundColor Cyan
New-Item -ItemType Directory -Force $dist | Out-Null
foreach ($bin in $bins) {
    Copy-Item (Join-Path $root "target\release\$bin") $dist -Force
}

# Folders are mirrored; user_library in dist is left alone: it holds the user's own blocks.
# The libraries used to live in Flprog_Network; drop the old copy from dist.
if (Test-Path (Join-Path $dist 'Flprog_Network')) { Remove-Item (Join-Path $dist 'Flprog_Network') -Recurse -Force }
foreach ($dir in 'library', 'examples', 'libs') {
    $src = Join-Path $root $dir
    $dst = Join-Path $dist $dir
    if (-not (Test-Path $src)) {
        Write-Host "Пропуск: нет папки $dir" -ForegroundColor Yellow
        continue
    }
    # .flp holds per-project arduino-cli build caches: hundreds of MB, not for distribution.
    & robocopy $src $dst /MIR /XD .flp /NFL /NDL /NJH /NJS /NP | Out-Null
    # robocopy: codes below 8 mean success.
    if ($LASTEXITCODE -ge 8) { Fail "не удалось скопировать $dir (robocopy $LASTEXITCODE)." }
    # Excluded dirs are also kept from purging, so drop ones left from earlier copies.
    Get-ChildItem $dst -Recurse -Directory -Force -Filter .flp | Remove-Item -Recurse -Force
}
Copy-Item (Join-Path $PSScriptRoot 'dist-README.txt') (Join-Path $dist 'README.txt') -Force

if (-not $NoZip) {
    Write-Host '== Архив dist\FLP_next.zip ==' -ForegroundColor Cyan
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    if (Test-Path $zip) { Remove-Item $zip -Force }
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $dist, $zip, [System.IO.Compression.CompressionLevel]::Optimal, $true)
}

Write-Host ''
Write-Host "Готово: $dist" -ForegroundColor Green
if (-not $NoZip) { Write-Host "Архив:  $zip" -ForegroundColor Green }
Start-Editor
