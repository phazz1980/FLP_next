# Release build of FLP_next and assembly of the distribution folder and archive.
# Usage: build.cmd (double-click) or `powershell -File scripts\build-dist.ps1 [-NoZip]`.
param([switch]$NoZip)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist\FLP_next'
$zip = Join-Path $root 'dist\FLP_next.zip'
$bins = 'flp-editor.exe', 'flp-mcp.exe', 'flp.exe'

function Fail($msg) {
    Write-Host "ОШИБКА: $msg" -ForegroundColor Red
    exit 1
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Fail 'cargo не найден. Установите Rust: https://rustup.rs (toolchain MSVC).'
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

$running = Get-Process -Name ($bins | ForEach-Object { $_ -replace '\.exe$' }) -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($dist, [StringComparison]::OrdinalIgnoreCase) }
if ($running) {
    Fail "закройте запущенные из dist программы: $(($running.Name | Sort-Object -Unique) -join ', ')."
}

Write-Host '== Сборка папки dist\FLP_next ==' -ForegroundColor Cyan
New-Item -ItemType Directory -Force $dist | Out-Null
foreach ($bin in $bins) {
    Copy-Item (Join-Path $root "target\release\$bin") $dist -Force
}

# Folders are mirrored; user_library in dist is left alone: it holds the user's own blocks.
foreach ($dir in 'library', 'examples', 'Flprog_Network') {
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
