# Runs scenario.txt on the board (module counter.wasm via the master's in/param/tick/out
# commands) and compares every `out` line with golden.txt made by the flp-sim test.
# Usage: compare.ps1 -Port COM10 -Wasm <counter.wasm> (flp upload-wasm is used for the upload)
param([string]$Port = 'COM10', [string]$Wasm = ($env:TEMP + '\mods\counter.wasm'), [string]$Flp = 'cargo run -q -p flp-cli --')
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Invoke-Expression "$Flp upload-wasm `"$Wasm`" --name counter --port $Port"
if ($LASTEXITCODE -ne 0) { exit 1 }
$golden = @(Get-Content "$here\golden.txt" | Where-Object { $_.Trim() })
$p = New-Object System.IO.Ports.SerialPort $Port, 115200
$p.ReadTimeout = 5000; $p.NewLine = "`n"; $p.DtrEnable = $false; $p.RtsEnable = $false
$p.Open(); Start-Sleep -Milliseconds 200; $p.DiscardInBuffer()
function Send([string]$line) {
    $p.WriteLine($line); $got = @()
    while ($true) { $l = $p.ReadLine().Trim(); if ($l -eq 'ok') { return $got }; if ($l.StartsWith('err')) { throw "$line -> $l" }; $got += $l }
}
$out = @()
foreach ($raw in Get-Content "$here\scenario.txt") {
    $line = $raw.Trim()
    if (-not $line -or $line.StartsWith('#')) { continue }
    $w = $line -split '\s+'
    if ($w[0] -eq 'out') { $out += Send "out counter 4" } else { [void](Send ("$line" -replace '^(reset|in|param|tick)', '$1 counter ')) }
}
$p.Close()
$bad = 0
for ($i = 0; $i -lt $golden.Count; $i++) {
    if ($out[$i] -ne $golden[$i]) { $bad++; "MISMATCH #$i`n  board: $($out[$i])`n  golden: $($golden[$i])" }
}
if ($out.Count -ne $golden.Count) { $bad++; "line count: board $($out.Count), golden $($golden.Count)" }
if ($bad) { "FAILED: $bad" ; exit 1 } else { "OK: $($golden.Count) lines are bit-identical" }
