param(
    [Parameter(Mandatory)][string]$BeforeExe,
    [Parameter(Mandatory)][string]$AfterExe,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(3,15)][int]$Rounds = 7
)
$ErrorActionPreference = 'Stop'
$executables = @{
    before = (Resolve-Path -LiteralPath $BeforeExe).Path
    after = (Resolve-Path -LiteralPath $AfterExe).Path
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$records = [Collections.Generic.List[object]]::new()
$contracts = @{}
$culture = [Globalization.CultureInfo]::InvariantCulture
for ($round = 1; $round -le $Rounds; $round++) {
    $order = if ($round % 2) { @('before','after') } else { @('after','before') }
    foreach ($variant in $order) {
        $lines = @(& $executables[$variant])
        if ($LASTEXITCODE -ne 0) { throw "Benchmark failed: $variant round $round" }
        $lines | Set-Content -LiteralPath (Join-Path $OutputDirectory "$variant-$round.txt") -Encoding utf8NoBOM
        foreach ($line in $lines) {
            $fields = @{}
            foreach ($match in [regex]::Matches($line, '(\w+)=([^ ]+)')) {
                $fields[$match.Groups[1].Value] = $match.Groups[2].Value
            }
            if ($fields.ContainsKey('case')) { $scenario = $fields.case; $metric = 'ns_per_sample'; $unit = 'ns/sample' }
            elseif ($fields.ContainsKey('raw_packets')) { $scenario = 'raw_8000_hz'; $metric = 'ns_per_packet'; $unit = 'ns/packet' }
            elseif ($fields.ContainsKey('raster_frames')) { $scenario = 'raster'; $metric = 'us_per_frame'; $unit = 'us/frame' }
            else { throw "Unrecognized benchmark output: $line" }
            if ($fields.allocations -ne '0') { throw "Hot path allocated: $scenario" }
            $contract = (@('samples','raw_packets','raster_frames','triggers') | ForEach-Object { "$_=$($fields[$_])" }) -join ';'
            if ($contracts.ContainsKey($scenario) -and $contracts[$scenario] -ne $contract) {
                throw "Input count or trigger result changed: $scenario"
            }
            $contracts[$scenario] = $contract
            $records.Add([pscustomobject]@{
                round=$round; variant=$variant; scenario=$scenario; unit=$unit
                value=[double]::Parse($fields[$metric], $culture)
            })
        }
    }
    Write-Output "Completed benchmark pair $round/$Rounds"
}
function Median($values) {
    $ordered = @($values | Sort-Object)
    $middle = [int][Math]::Floor($ordered.Count / 2)
    if ($ordered.Count % 2) { return $ordered[$middle] }
    return ($ordered[$middle - 1] + $ordered[$middle]) / 2
}
$summary = @(
    foreach ($scenario in @($records.scenario | Select-Object -Unique)) {
        $before = @($records | Where-Object { $_.scenario -eq $scenario -and $_.variant -eq 'before' })
        $after = @($records | Where-Object { $_.scenario -eq $scenario -and $_.variant -eq 'after' })
        if ($before.Count -ne $Rounds -or $after.Count -ne $Rounds) { throw "Missing benchmark cases: $scenario" }
        $beforeMedian = Median $before.value
        $afterMedian = Median $after.value
        [pscustomobject]@{
            scenario=$scenario; unit=$before[0].unit
            beforeMedian=$beforeMedian; afterMedian=$afterMedian
            changePercent=[Math]::Round(100 * ($afterMedian / $beforeMedian - 1), 2)
            beforeMin=($before.value | Measure-Object -Minimum).Minimum
            beforeMax=($before.value | Measure-Object -Maximum).Maximum
            afterMin=($after.value | Measure-Object -Minimum).Minimum
            afterMax=($after.value | Measure-Object -Maximum).Maximum
        }
    }
)
[ordered]@{
    rounds=$Rounds
    beforeSha256=(Get-FileHash -LiteralPath $executables.before -Algorithm SHA256).Hash
    afterSha256=(Get-FileHash -LiteralPath $executables.after -Algorithm SHA256).Hash
    summary=$summary; runs=@($records.ToArray())
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'comparison.json') -Encoding utf8NoBOM
$summary | Format-Table scenario,unit,beforeMedian,afterMedian,changePercent
