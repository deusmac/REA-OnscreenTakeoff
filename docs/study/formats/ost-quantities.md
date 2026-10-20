# OST quantities: geometry, scale and stored totals

Our notes on how an OST job database stores takeoff geometry and how its stored totals
follow from it. Source: a copy of the OST sample-projects database (28 bids, 223 pages,
1004 conditions, 14963 takeoffs, 1253 total rows), read with `tools/ost-answer-key.ps1`
(numbers and codes only) and checked by `app/crates/core/tests/ost_parity.rs`. JC allowed
reading his own OST files for compatibility on 2026-10-20 (CLAUDE.md section 5). No raw
job data is in the repo.

## Units: everything is stored in real inches

- Takeoff geometry and every stored total are in real-world (building) units at base
  precision: inches, square inches, cubic inches, or plain counts.
- The UOM columns on a condition (UOM1 to UOM3) only pick the display unit. The stored
  value is the same whether the UOM is linear feet, square feet, square yards or cubic
  yards. Observed UOM codes and what they sit next to: 0 each, 1 a length (only on count
  Total Height; unit unclear), 2 linear feet, 5 square feet, 6 square yards (one area
  condition), 9 cubic yards, -1 slot unused. Display conversion is ours to apply: divide
  by 12, 144, 1296 or 46656.

## Position encoding

`BidTakeoffs.Position` is a Binary column, but its bytes are plain ASCII text, numbers
separated by `;`, ending in a line feed. No UTF-16, no packed floats. Layout by condition
type (`BidConditions.Type`):

- Type 0, linear: one takeoff is one segment: `x1;y1;x2;y2;s` where `s` is the signed arc
  height (sagitta) of a curved segment, 0 for straight. 627 takeoffs carry two extra
  trailing zeros (`x1;y1;x2;y2;0;0;0`); they did not change any total. A drawn run with N
  segments is N takeoff rows.
- Type 1, area: `x;y;x;y;...` one closed ring, first point not repeated.
- Type 2, count: `x;y`.
- Type 3, attached count (doors, windows on a wall): `d;side;x;y`, where `ParentUID` is the
  linear takeoff it sits on, `d` looks like the distance along that segment and `side` is
  0 or 1. Only the parent link and the condition dimensions matter for totals.

Coordinates are real inches, not sheet units: at 1/4" = 1'-0" a 48.76 in wide sheet is
2340 real inches wide and its points stay under 1760; at 1" = 100' points reach 15400 on a
17 in sheet. That is also why the XML export looked like 1/100 inch of sheet: at the most
common 1/8" scale one real inch is 1/96 of a sheet inch.

Other takeoff columns that feed totals: `IsNegativeQuantity` (subtract), `Count` (the
drop length in inches when the condition has Drop/Run on), `ParentUID` (backout or
attachment parent). `Quantity` was 0 in every row.

## Scale math

`BidPages.ScaleFactor1` sheet inches equal `ScaleFactor2` real inches, for every style:

- Architectural (ScaleStyle 1): 1/8" = 1'-0" is 0.125 and 12; 3/32" is 0.09375 and 12.
- Civil (ScaleStyle 2): 1" = 20' is 1 and 240; 1" = 100' is 1 and 1200.
- Custom (IsCustomScale): 1 and a measured value such as 17.1895.

Real inches per sheet inch = ScaleFactor2 / ScaleFactor1, so real inches per PDF point =
ScaleFactor2 / ScaleFactor1 / 72. In our core that is
`Scale::architectural(ScaleFactor1, ScaleFactor2 / 12)`. `Scale` was 0 everywhere; Bids
and Settings carry the default scale for new pages. Because points are stored already
scaled, the scale never enters a stored total; it only matters to place points on the
PDF (sheet points = real inches * ScaleFactor1 / ScaleFactor2 * 72, not yet checked
against a rendered sheet).

## Result codes (`BidConditions.Quantity1` to `Quantity3`)

Each stored value is in base units. L = run length, H = height, T = thickness, W, D =
width, depth, C = net count, A = area, BA = backout area, P = outer perimeter, BL = backout
perimeter. Count after each code is how many stored quantities checked it.

Linear (type 0):
- 1 Length: L minus full-height openings (480).
- 2 Segment count: one per takeoff row (146).
- 3 Surface area one side: H * L minus openings (204).
- 4 Surface area both sides: 2 * (H * L minus openings) (18).
- 5 Top or bottom: T * L (2).
- 9 All sides: 2 * (H + T) * L (8).
- 20 Volume: H * T * L (7).
- 6, 7, 8 (top and bottom, one end, both ends) were not used by any condition.

Area (type 1):
- 11 Area: (A - BA) * slope factor (244).
- 12 Area: same value as 11 in all 3 rows; none had backouts or attachments, so it can be
  "minus attachments" (3).
- 14 Perimeter without backouts: P (2).
- 15 Perimeter: P + BL (115).
- 16 Grid length: not reproduced, see open items (9).
- 17 Tile count, average: area / (GridSize1 * GridSize2), fractional (2).
- 19 Area count: number of area objects (6).
- 21 Volume: (A - BA) * slope * T (50).
- 33 Perimeter, same as 15 in its only row (1).
- 34 Area ignoring backouts: A * slope, checked on a row that has backouts (1).

Count (type 2) and attached count (type 3):
- 23 Count: C (567).
- 24 Total height: C * H (34).
- 5 Top or bottom: C * W * D (12).
- 13 Perimeter: C * 2 * (W + D) (14).
- 20 Volume: C * H * W * D, but a circle (Shape 1) is a cylinder, C * H * pi * W^2 / 4
  (11).
- 31 Opening perimeter, four sides: C * 2 * (H + W) (2).
- 32 Opening perimeter, top and both sides: C * (2 * H + W) (33).

Count shapes seen: 1 circle, 3 rectangle (own depth); every other shape uses D = W. Codes
4 to 11 and 14 look like door and window shapes (they carry codes 31 and 32).

## How a total row is built

`BidTakeoffTotals` has one row per bid, page, zone, bid area, typical area and condition.
Quantity1 to Quantity3 hold the results above; SumQuantity1 to SumQuantity3 were equal to
them in every row.

- Net total: positive takeoffs minus takeoffs with `IsNegativeQuantity`.
- Slope: factor sqrt(1 + (Rise / Run)^2), on area and volume and on linear length, never
  on area perimeter. Run 0 means flat.
- Area backouts: an area takeoff with `ParentUID` is a hole in its parent. Its area and
  perimeter count with the parent's condition total, even when the backout itself belongs
  to another condition. A `ParentUID` that points into another bid (left over from copying
  a bid) is ignored.
- Wall openings: an attached count whose condition has `Backout` on removes W * min(Hopen,
  H) from each wall side. An opening at least as tall as the wall also removes W from the
  length. A zero-height opening removes nothing.
- Drops: with Drop/Run on, each segment's `Count` value (inches) is added to the length.
- Curved segments: length is the arc length from chord and sagitta (major arc when the
  sagitta exceeds half the chord). Using the chord instead breaks 20 stored totals.
- Typical areas: a row with both a bid area and a typical area equals the typical area's
  own row (bid area empty) times `BidTypAreaCounts.Count` for that bid area and typical
  area.

## Match rate (sample database, 2026-10-20)

Tolerance 0.01 absolute or 1% relative, per stored quantity.

- Linear: 865 of 865 quantities, 477 of 477 rows.
- Area: 424 of 433 quantities, 263 of 272 rows. All 9 misses are grid length (code 16).
- Count: 575 of 575 quantities, 429 of 429 rows.
- Attached count: 98 of 98 quantities, 75 of 75 rows.
- Total: 1962 of 1971 quantities, 1244 of 1253 rows.

## Count and attached count: codes 13, 25, 26 and stale rows (2026-10-20)

Checked against all 16 job databases (count 5170 of 5173 quantities, attached count 423 of
425 after these rules).

- Code 25 is C * W * H (one width side) and code 26 is 2 * C * W * H (both width sides).
  Depth never enters: a 36 x 84 object stores 3024 and 6048, a 60 x 84 x 12 object stores
  10080 for code 26, and attached 108 x 132 stores 14256 for code 25. Both are the width
  side results we already had under other names.
- Count shape 12 has a perimeter (code 13) of 4 * W / sqrt(3) per object, not 2 * (W + D):
  20 objects of W 36, H 216, D 0 store 1662.769 (two conditions, same numbers). Which
  figure shape 12 is stays unknown; only this perimeter is evidenced, so other results for it
  use the generic formulas.
- Code 13 for shape 0 (square) is 4 * W and for shape 3 (rectangle) is 2 * (W + D); both
  held in every row.
- Two attached count total rows (one each for a 312 x 120 and a 216 x 120 opening on one page)
  store a count of 1 but their conditions have no takeoff rows at all. That is a stale total
  OST kept after its takeoffs went; nothing in the takeoffs can reproduce it. The same
  database has count totals where OST stores 0 and takeoffs exist, which is a different
  problem (rows OST did not refresh).

## Open items

- Grid length (area code 16). Clipping grid lines at GridSize1 and GridSize2 spacing to the
  area gets within about 1 to 7% but never exact. Candidates tried for the grid origin:
  world zero, the area's bounding-box corner, its first vertex, each plus
  GridOffsetX/GridOffsetY. Hypotheses: the origin is the page or the area center, the
  takeoff's GridRotation rotates the lines, the perimeter is subtracted or added, or Gap
  (one row) shortens lines at each crossing.
- Codes with no data in the sample: linear 6, 7, 8; area 18 (likely visible tile count);
  count 26. Attachments (area AA, AL, AV) have no rows either.
- Whether openings change linear results other than length and side areas, whether slope
  applies to drops, and whether slope applies to backout areas: no sample row tells.
- UOM code 1: a length unit used only for count Total Height.
- Custom-scale pages: on one custom page the points run past the sheet size in real
  inches, so the point origin or the custom scale reading may differ there.
- Sheet placement of points (origin corner, axis direction, rotation) is not yet checked
  against a rendered PDF.
