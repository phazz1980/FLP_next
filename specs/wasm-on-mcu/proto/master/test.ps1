# Drives the prototype master over Serial: uploads blink and hang, checks the watchdog,
# then does 1000 hot swaps and compares the heap. Usage: test.ps1 -Port COM10 -Dir <folder with blink.wasm, hang.wasm>
param([string]$Port = 'COM10', [string]$Dir = $env:TEMP + '\mods', [int]$Swaps = 1000)

$table = for ($i = 0; $i -lt 256; $i++) {
    $c = [uint64]$i
    for ($k = 0; $k -lt 8; $k++) { if ($c -band 1) { $c = [uint64]3988292384 -bxor ($c -shr 1) } else { $c = $c -shr 1 } }
    $c
}
function Crc32([byte[]]$b) {
    $c = [uint64]4294967295
    foreach ($x in $b) { $c = $table[[int](($c -bxor $x) -band 255)] -bxor ($c -shr 8) }
    return [uint64]($c -bxor 4294967295)
}

$p = New-Object System.IO.Ports.SerialPort $Port, 115200
$p.ReadTimeout = 3000; $p.NewLine = "`n"; $p.DtrEnable = $false; $p.RtsEnable = $true
$p.Open(); Start-Sleep -Milliseconds 100; $p.RtsEnable = $false; Start-Sleep -Milliseconds 2500
$p.DiscardInBuffer()

function Cmd([string]$line, [string[]]$stop = @('end', 'ok', 'err')) {
    $p.WriteLine($line); $out = @()
    try { while ($true) { $l = $p.ReadLine().Trim(); $out += $l; if ($stop | Where-Object { $l -eq $_ -or $l.StartsWith("$_ ") }) { break } } } catch { $out += 'TIMEOUT' }
    return $out
}
function Put([string]$name, [string]$allow) {
    $b = [IO.File]::ReadAllBytes("$Dir\$name.wasm")
    $p.WriteLine(("put {0} {1} {2:x8} {3}" -f $name, $b.Length, (Crc32 $b), $allow))
    $first = $p.ReadLine().Trim()
    if ($first -ne 'ready') { return $first }
    $p.Write($b, 0, $b.Length)
    $out = @(); try { while ($true) { $l = $p.ReadLine().Trim(); $out += $l; if ($l -eq 'ok' -or $l.StartsWith('err')) { break } } } catch { $out += 'TIMEOUT' }
    return $out
}

"== put blink";  Put 'blink' 'gpio_write,millis'
"== put hang";   Put 'hang' '-'
Start-Sleep -Seconds 2
"== list"; Cmd 'list'
"== log";  Cmd 'log'
"== list again (blink must keep stepping)"; Cmd 'list'

"== heap before swaps"; $before = Cmd 'stats'; $before
$t = Get-Date
$fail = 0
for ($i = 0; $i -lt $Swaps; $i++) { $r = @(Cmd 'swap blink'); if ($r[-1] -ne 'ok') { $fail++ } }
"swaps: $Swaps, failed: $fail, seconds: $([int]((Get-Date) - $t).TotalSeconds)"
"== heap after swaps"; Cmd 'stats'
$p.Close()
