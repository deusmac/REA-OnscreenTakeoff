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

- Type 0, linear: one takeoff is one segment, in one of two layouts: `x1;y1;x2;y2;s` or
  `x1;y1;x2;y2;px;py;s`. `s` is the signed arc height (sagitta) of a curved segment, 0 for
  straight, and it is always the last number. In the seven-number layout `px;py` is a point,
  not the sagitta (see "Linear rules from JC's databases" below). A drawn run with N
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
- 8 Both ends: 2 * H * T per segment (2 rows in two copies of the sample database: a
  two-segment run stored 2 * 180 * 4 * 2 = 2880).
- 6 and 7 (top and bottom, one end) were not used by any condition in any database.

Area (type 1):
- 11 Area: (A - BA) * slope factor (244).
- 12 Area: same value as 11 in all 3 rows; none had backouts or attachments, so it can be
  "minus attachments" (3).
- 14 Perimeter without backouts: P (2).
- 15 Perimeter: P + BL (115).
- 16 Grid length: length of the tile grid lines inside the area, see "Gridded areas and
  Round Quantity" below.
- 17 Tile count, average: area / (GridSize1 * GridSize2), fractional (2). With Round
  Quantity on, the area here is the rounded area.
- 18 Tile count: whole tiles touched by the area, see below.
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

## Gridded areas and Round Quantity

Found on JC's 16 job databases (2026-10-20). Conditions: `Grid`, `GridSize1`, `GridSize2`,
`Gap`, `RoundQuantity`, `RoundUp`. Takeoffs: `GridOffsetX`, `GridOffsetY`, `GridRotation`.

Grid placement:
- Origin: the area centroid of the takeoff's outer ring (not the first click, not the
  bounding box corner or centre) plus (`GridOffsetX`, `GridOffsetY`) in page inches.
- Axes: turned by `GridRotation` (radians) about that origin. `GridSize1` is the tile size
  along the turned x axis, `GridSize2` along the turned y axis.
- Line pitch: tile size plus `Gap` (16 in tiles with Gap 1 give lines 17 in apart).
- Lines sit at every whole multiple of the pitch from the origin, so offsets larger than a
  tile simply wrap.

Codes:
- 16 Grid length: total length of grid lines inside the area (outer ring minus
  backouts), both directions. 218 of 221 stored rows match, Gap row included.
- 18 Tile count: number of grid cells that hold any part of the area; a cut tile counts
  whole, a cell entirely inside a backout does not count. 65 of 73 rows match.
- 17 Tile count, average: area / tile area as before (no grid placement involved).
- Grid length and tile count use the drawn shape, not the Round Quantity shape.

Round Quantity (`RoundQuantity` on, `RoundUp` = step in inches, seen only on area
conditions with grid on, plus 4 linear conditions not checked):
- Perimeter: every edge of the closed outline, closing edge included, rounded up to the
  next multiple of the step, then summed. Backout perimeters are added unrounded.
- Area: start at the first vertex and walk the drawn edges in order, keeping each edge's
  direction but rounding its length up to the step. The closing edge is not walked; the
  outline closes straight from the last walked point to the start, even when the walk does
  not come back to it. Backout areas are subtracted unrounded. A 169.142 x 102.664 in room
  with a 24 in step stores 192 x 120 = 23040 sq in; an L-shape with edges 64, 93, 149,
  193, 213, 100 stores 43488.
- Tile count average then divides the rounded area by the tile area.

Residuals (not explained yet):
- Round Quantity area on 6 totals (about 18 quantities): shapes whose closing edge is
  diagonal (for example a last edge of 8.06 in at a slant), or whose short jogs round up
  so the walk overshoots. OST's value differs from the walk by 1 to 6%. Snapping vertices
  to a step grid, walking from other start vertices or in reverse, and walking the closing
  edge were all tried and none fits every row.
- Tile count with a backout (3 copies of one total): ours 1091, OST 1107; with the
  backout ignored we get 1251. Placing the grid from the net centroid made it worse.
- Tile count on a total mixing turned and plain grids: ours 528, OST 530.
- Grid length on one total with a backout: 0.09% off.
- Rows where OST stores 0 but the takeoffs give a value (stale totals, 2 grid length and
  4 tile count rows) are not grid issues.

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

## Linear rules from JC's databases (2026-10-20)

Checked on the 16 job databases in JC's corpus (14 with totals rows, 26,001 stored linear
quantities). Before these rules linear matched 25,367 (97.6%); after, 25,946 (99.8%). After
the re-export and the stale-row bucket, 25,946 of 26,000 compared.

- Seven-number positions. 164,624 linear takeoffs use `x1;y1;x2;y2;px;py;s`. On 2,636
  rotated takeoffs (`Rotation` not 0) `px;py` is the rotation pivot, often far from the
  segment; on 234 curved ones it is a point near the arc. Reading `px` as the sagitta made a
  16 in segment into a near-full circle: 8 such segments stored 128 in, and we got 35,290.
  The sagitta is the last number in both layouts. This one rule fixed the worst rows in
  OST_Projects 20252 (ratios up to 215) and most misses in JC_Commercial.
- Typical groups. A takeoff with `TypGroupUID` set is not part of `BidTakeoffTotals`. Pages
  that hold only typical-group takeoffs have no totals rows for them at all, and on 5 rows
  that mix both kinds, the stored total is exactly the takeoffs without a typical group
  (for example 91.6 in from one plain segment beside 26 typical-group segments). Those
  quantities probably live in `BidTypicalGroupTotals` (empty in every database read so
  far). The harness applies it to every type; it also fixed 12 area quantities.
- Zone outlines. The zone region of the zero-rows section below is stored as a takeoff
  with a zone id and no condition; its points are the zone ring. Every takeoff behind the
  24 zone rows in the corpus (20 linear, 4 area) lies fully inside its page's ring, so the
  harness now assigns each takeoff to the zone whose ring holds all its points (its own
  `BidZoneUID` wins when set). A zone row reads the takeoffs inside, signed by
  `IsNegativeValues`; the no-zone row reads the rest, which is empty in every case and so
  stores 0. No takeoff crosses an outline, so how OST splits one is unknown.

Residuals (55 quantities, none explained by geometry):

- Stale heights (48 quantities in OST_Projects 20252 and JC_Commercial). Length matches
  but side area equals length times a different round height (36, 42, 132, 146, 456 in
  where the condition now says 48, 60, 144, 156 or 480). One condition has two rows, one
  at its current height and one at an earlier one, so OST kept the old total on rows it
  did not recompute. Nothing in the database holds the earlier height.
- One JC_Commercial row is 300 in longer in our sum than stored, with area consistent
  with the stored length; likely also stale.
- One OST_Projects 20252 row stores 175,161 in for 409 in of segments, plus nonzero values
  in its two unused slots (the condition was likely changed after takeoff).
- One JOHN_CP row with no takeoffs left for its condition on that page (deleted takeoffs,
  stored total kept).

## Open items

- Round Quantity area on some non-rectangular shapes, and tile count with a backout; see
  the residual list in "Gridded areas and Round Quantity".
- Round Quantity on linear conditions (seen on 4 linear conditions) is not checked yet.
- Codes no condition uses in JC's databases: linear 6, 7. Attachments (area AA, AL, AV) have
  no rows either.
- Whether openings change linear results other than length and side areas, whether slope
  applies to drops, and whether slope applies to backout areas: no sample row tells.
- UOM code 1: a length unit used only for count Total Height.
- Custom-scale pages: on one custom page the points run past the sheet size in real
  inches, so the point origin or the custom scale reading may differ there.
- Sheet placement of points (origin corner, axis direction, rotation) is not yet checked
  against a rendered PDF.

## Rows where OST stores zero (zones and stale rows)

Checked 2026-10-20 on all 16 of JC's job databases. Two separate causes, neither related to
hidden pages, hidden layers, template conditions or the show-takeoff flag (those only change
what is drawn; their totals are stored normally), and no bid in the corpus has the recalc flag
set.

Zone rule (explains every "OST 0, ours big" row in JC_Commercial and JOHN_CP, 57 rows):
- A zone is a region drawn on a page, not an attribute of a takeoff. Takeoff rows carry an
  empty zone id even when OST puts their quantity under a zone.
- When a page and condition has a zone totals row, that row holds the whole quantity of the
  page and condition, with the sign flipped if the zone is marked negative (IsNegativeValues).
  The sibling row with no zone then stores Quantity 0 for every slot (nothing is left over
  in these jobs) and keeps the real number in SumQuantity, as the sum of itself and its zone
  rows. Zone rows themselves store SumQuantity 0.
- This is the whole meaning of SumQuantity differing from Quantity: 102 slots in
  JC_Commercial and 12 in JOHN_CP, all of them zone siblings. No other database has a
  difference except stale rows below.
- Harness: each takeoff gets the zone whose outline holds it (see "Zone outlines" under the
  linear rules), a zone row is compared against its signed takeoffs, and a sibling with no
  takeoffs left must store zero. Every sibling in the corpus is zero.
  Earlier these zone rows were "not modeled" or showed as a large OST 0 versus ours mismatch.

Stale rows (3 rows, all in OST_Projects__20252, count and attached-count conditions):
- OST stored 0 in every used slot but a non-zero value in SumQuantity of a slot whose unit is
  "unused" (-1), for example Sum3 equal to the takeoff count. A value in an unused slot means
  the row was written under an older definition of the condition and never refreshed.
  Normal count rows always have Sum equal to Quantity and nothing in unused slots.
- Evidence this is not our math: the takeoffs exist and the sibling rows on the same page
  match; the row cannot be reproduced from the condition as it is stored now.
- Harness: a row with a mismatch and a non-zero value in an unused slot is reported as
  "stale in OST" and left out of the compared rows and quantities.

Not found: the brief also named 102 SumQuantity differences in OST_Projects__20252. In the
re-export that database has none; the 102 belong to JC_Commercial (zone siblings above).
Exporter now also writes layers.csv and the flags RecalcNeeded, page Show, condition
IsTemplate, ShowTakeoff and BidLayerUID so these rules can be re-tested.
