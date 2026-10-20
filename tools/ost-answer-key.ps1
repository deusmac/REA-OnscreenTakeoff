# Exports an OST job database (.mdb, a COPY) to numeric CSV files for the parity test
# (app/crates/core/tests/ost_parity.rs). Numbers and codes only: no names, notes, paths,
# employees or passwords. Output goes outside the repo; never commit it (client data).
#
# Usage (64-bit Windows PowerShell, needs the ACE OLEDB 16.0 provider):
#   .\tools\ost-answer-key.ps1 -Mdb "E:\opentakeoff-study\ost\copy.mdb" [-Out E:\opentakeoff-study\answer-key]
param(
    [Parameter(Mandatory = $true)][string]$Mdb,
    [string]$Out = 'E:\opentakeoff-study\answer-key'
)
$ErrorActionPreference = 'Stop'
if (-not [Environment]::Is64BitProcess) { throw 'Run from 64-bit PowerShell (ACE provider is 64-bit).' }
if (-not (Test-Path -LiteralPath $Mdb)) { throw "Not found: $Mdb" }
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
New-Item -ItemType Directory -Force -Path $Out | Out-Null
if ((Resolve-Path $Out).Path.StartsWith($repo, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Output folder is inside the repo; pick a folder outside it: $Out"
}

$conn = New-Object System.Data.OleDb.OleDbConnection("Provider=Microsoft.ACE.OLEDB.16.0;Data Source=$Mdb;Mode=Read")
$conn.Open()
function Query([string]$sql) {
    $cmd = $conn.CreateCommand(); $cmd.CommandText = $sql
    $t = New-Object System.Data.DataTable
    [void](New-Object System.Data.OleDb.OleDbDataAdapter($cmd)).Fill($t)
    , $t
}
# Writes a DataTable as CSV. Booleans become 0/1, doubles use invariant round-trip text.
function Save([System.Data.DataTable]$t, [string]$name) {
    $inv = [Globalization.CultureInfo]::InvariantCulture
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add((($t.Columns | ForEach-Object { $_.ColumnName }) -join ','))
    foreach ($r in $t.Rows) {
        $cells = foreach ($c in $t.Columns) {
            $v = $r[$c.ColumnName]
            if ($v -is [DBNull]) { '' }
            elseif ($v -is [bool]) { [int]$v }
            elseif ($v -is [double]) { $v.ToString('R', $inv) }
            else { [string]$v }
        }
        $lines.Add(($cells -join ','))
    }
    [IO.File]::WriteAllLines((Join-Path $Out $name), $lines)
    "{0,-16} {1,6} rows" -f $name, $t.Rows.Count
}

Save (Query 'SELECT UID, MeasureBase, ScaleStyle, IsCustomScale, ScaleFactor1, ScaleFactor2, PageScale, TakeoffIncrements, TypicalType, IgnoreBidAreas, RecalcNeeded, IsCalculatedForSlope, IsCalculatedForLaborCostCodeTotals FROM Bids') 'bids.csv'
Save (Query 'SELECT UID, BidUID, ScaleStyle, IsCustomScale, ScaleFactor1, ScaleFactor2, Scale, Width, Height, Rotation, MasterPageUID, TypicalPageRepeats, Show FROM BidPages') 'pages.csv'
Save (Query 'SELECT UID, BidUID, IsNegativeValues FROM BidZones') 'zones.csv'
Save (Query 'SELECT UID, BidUID, ParentUID FROM BidAreas') 'areas.csv'
Save (Query 'SELECT UID, BidUID, IsTemplate, Show, IsLocked FROM BidLayers') 'layers.csv'
Save (Query 'SELECT UID, BidAreaUID, BidTypAreaUID, [Count] FROM BidTypAreaCounts') 'typareacounts.csv'
Save (Query ('SELECT UID, BidUID, Type, Shape, Width, Height, Spacing, Thickness, Rise, Run, Depth, ' +
    'UOM1, UOM2, UOM3, Quantity1, Quantity2, Quantity3, RoundUp, RoundQuantity, Backout, DropRun, DropValue, ' +
    'Grid, GridSize1, GridSize2, Gap, ManualLength, Curve, IsCurvedSegment, BFperLF, ' +
    'DirectQuantity1, DirectQuantity2, DirectQuantity3, BidLayerUID, IsTemplate, ShowTakeoff FROM BidConditions')) 'conditions.csv'
Save (Query 'SELECT * FROM BidTakeoffTotals') 'totals.csv'

# Takeoffs: Position is a Binary column holding ASCII text (see
# docs/study/formats/ost-quantities.md). Rows inside it are split by LF; we keep them
# joined with '|' so the CSV stays one line per takeoff and has no commas.
$t = Query ('SELECT UID, BidUID, BidPageUID, BidConditionUID, BidZoneUID, BidAreaUID, BidTypAreaUID, ' +
    'ParentUID, [No], Quantity, [Count], Rotation, GridOffsetX, GridOffsetY, GridRotation, IsNegativeQuantity, Curve, TypGroupUID, Position FROM BidTakeoffs')
$t.Columns.Add('Points', [string]) | Out-Null
$bad = 0
foreach ($r in $t.Rows) {
    $p = $r['Position']
    if ($p -is [byte[]]) {
        $s = [Text.Encoding]::ASCII.GetString($p).TrimEnd("`n", "`r", [char]0)
        if ($s -notmatch '^[-0-9.;Ee+\r\n]*$') { $bad++; $s = '' }
        $r['Points'] = ($s -replace "`r", '') -replace "`n", '|'
    }
}
$t.Columns.Remove('Position')
Save $t 'takeoffs.csv'
if ($bad) { "warning: $bad Position values were not numeric text and were left empty" }
$conn.Close()

