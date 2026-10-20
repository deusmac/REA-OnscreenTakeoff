# OST project package format (.osp / .ost)

Our own notes from reading JC's copy of one of his own jobs (permission recorded in
CLAUDE.md section 5, 2026-10-20). No vendor help text, no raw job data. Importer:
`app/crates/ost` (package `rea-ost`). Status: observed on one job (13 pages, 42 conditions,
275 takeoffs, 1 alternate bid). Everything marked "inferred" needs more samples.

## Container

- `.osp` is a Microsoft Cabinet archive (starts with the bytes `MSCF`). Read in memory with
  the `cab` crate; nothing is unpacked to disk.
- Files inside:
  - one `<name>.ost` job file (XML, below);
  - `BidTrans.xml`, a tiny file holding a version and a GUID/send-date for the sync feature.
    Not needed to rebuild a job; ignored;
  - the plan files, each stored under `TempImages!.tmp\` followed by the original absolute
    path with the drive letter removed. Example: page path `E:\Desktop\A\B.pdf` is stored as
    `TempImages!.tmp\Desktop\A\B.pdf`. `plan_archive_path()` computes this.
- A bare `.ost` is the same XML without the cabinet. The importer sniffs `MSCF`.
- Safety: the importer refuses more than one `.ost` per package, and size-caps the job file
  and each plan file before reading.

## XML layout

Root `XML_ROOT`. Every value is an XML attribute; there is no text content. Line breaks in
free text appear as character references. Free text may contain stray non-UTF-8 bytes, so
the importer decodes lossily.

- `OST Version="..."`: writing application version (3.2.0 here).
- `Bid`: the main bid. Attributes are the bid row. Children: `BidPages`, `BidConditions`,
  `BidLayers`, `BidAreas`, `BidConditionFolders`, many empty or unused collections, and
  `ALTERNATES` holding more `Bid` elements with the same shape.
- `CdnTypes/CdnType` (root level, shared): condition type groups (UID, Name).
- `Employees`: contains a password attribute. The importer skips this whole subtree at the
  XML reader and never stores any of it.
- `JobStatuses`, `AccessLevels`: ignored.
- Cross references use numeric UIDs, written as attributes ending in `UID`.

## Mapped entities

Units: lengths on conditions are inches. Page Width and Height are inches. Takeoff
coordinates are described below. Anything not listed is kept verbatim as a string map next
to the mapped struct (`project_extra`, `page_extra`, `condition_extra`, `takeoff_raw`).

### Bid to Project

- `UID` to id. `JobName` to name, `BidNo`, `JobID` to job_no, `Notes`.
- `CreateDateTime` ("2026 5 13 0 10 47", year month day hour minute second) to ISO 8601.
- `MeasureBase`: 0 seen (imperial). 1 = metric is assumed, not seen.
- `SourceBidUID` (when non-empty) to origin id.
- Estimator is only a UID here and the names live in the skipped employee list, so the
  estimator field stays empty (`EstimatorUID` remains in `project_extra`).

### BidPage to Page

- `UID` id. `Name` to sheet_name, `SheetNo` to sheet_no, `ImagePath` to file_path (the
  original absolute path, not the archive path).
- `Index1`: 1-based page number inside the plan file (`MultiPageCount` = pages in that
  file). Page.file_page_index = Index1 - 1. Page.index is document order.
- `Width`, `Height`: sheet size in inches, times 72 for our PDF points.
- `Rotation`: degrees, must be a multiple of 90 (only 0 seen).
- Scale: `ScaleFactor1` paper inches stand for `ScaleFactor2` real inches. Example 0.125 and
  12 means 1/8 in = 12 in (1/8" = 1'-0"), 96 real inches per paper inch. Our value is real
  inches per point: `ScaleFactor2 / ScaleFactor1 / 72`. A page with a zero factor gets no
  scale. Seen: 0.125, 0.1875 and 0.25 over 12.
- `ScaleStyle`: 1 seen on every page and means architectural (inferred from the factors).
  Other codes are mapped to Custom and the raw code stays in the extra map. `IsCustomScale`
  1 forces Custom.
- Not mapped (kept raw): Sequence, Show, zoom and view position, flip, deskew, overlay
  fields, digitizer fields, OCR fields, GUID, MasterPageUID, TypicalPageRepeats, and the many
  annotation collections under the page (lines, ink, clouds, callouts, comments, texts,
  dimensions, highlights, legends).

### BidCondition to Condition

- `UID` id, `RefNo` to condition number (falls back to UID), `Name`.
- `Type`: 0 linear, 1 area, 2 count. Evidence: names and units. Type 0 conditions are wall
  runs and tape with a length unit; type 1 are net area and ceilings with an area unit; type
  2 are doors, corner beads and headers with a each or length unit. Any other value is an
  error.
- `Height`, `Thickness`, `Width`, `Depth`, `Rise`, `Run`: inches. Slope is set when rise or
  run is non-zero. Evidence for inches: a 16 ft wall type has Height 192.
- `ColorFill` to color. It is a Windows color number with red in the lowest byte (255 is
  red, 16711680 is blue). `ColorLine` was 0 everywhere and stays raw.
- `BidLayerUID` to layer name via `BidLayers/BidLayer`. `CdnTypeUID` to ctype name via
  `CdnTypes`.
- `Pattern`: numeric code, stored as its text in Condition.pattern. Meaning unknown.
- Result kinds. `Quantity1..3` are codes saying which result to show, with `UOM1..3` as the
  unit codes. Mapped, all inferred from names and units, none yet checked against OST totals:
  - linear 1 = length
  - area 11 = area, 15 = perimeter
  - count 23 = count, 24 = total height (a 12 ft corner bead count carries a length unit)
  Left unmapped (raw only): linear 3 (a surface quantity, used by both one-sided and
  two-sided wall names, so the sides are not decoded), area 17, and all UOM codes.
- `Shape` on count conditions (1, 2, 12 seen) is not decoded; Condition.count_shape is None.
- Not mapped: Spacing, folder, template flag, font, grid and snap settings, Backout, drop,
  trim, labor/material/sub amounts, Excel cell links, Notes, display flags.

### BidTakeoff to Takeoff

- Lives under `BidPage/BidTakeoffs`. Page is the parent page. `BidConditionUID` must exist
  (error otherwise). `BidAreaUID` 0 means none; a non-zero value must match a bid area.
- `IsNegativeQuantity`: 0 seen; any other non-empty value is read as negative.
- `Position`: semicolon-separated numbers, terminated by a line break, sometimes with a
  trailing semicolon too. Alternating x;y values:
  - count: one point (2 numbers);
  - area: the outline points (even count, at least 3 points); no hole or backout data seen;
  - linear: points followed by three extra numbers (an arc point x, y and a signed bulge).
    The three are all zero for a straight segment. Seen only with exactly 2 points.
    When non-zero (condition `Curve` flag 0, three samples) we put the chord in the polyline
    and keep the arc in `TakeoffRaw.arc`. Evidence: in the sample the arc point sits 29.9
    units from the chord and the bulge value is -29.9. Our geometry has no arcs yet.
- Coordinate unit: real-world inches, already scaled by the page scale (settled 2026-10-20 by
  the answer key in `ost-quantities.md`: 1962 of 1971 stored OST totals reproduced from these
  coordinates with no scale applied). An earlier guess of hundredths of a sheet inch was close
  only because the sample page is 1/8 in = 1 ft (96 real in per sheet inch). The importer
  converts to sheet points with `value * ScaleFactor1 / ScaleFactor2 * 72`. Origin and y
  direction still unchecked against a rendered PDF (assumed top-left, y down).
- `Quantity` on the takeoff was 0 on every row, so OST does not store per-shape results.
  Totals have to be recomputed from geometry and scale, which is what rea-core does.
- Not mapped: Count, Rotation, `No`, `BidZoneUID`, `BidTypAreaUID`, typical-group fields,
  fonts, grid offsets, flips, `Curve` flag, GUID, modified flag.

### Totals input (`totals_job`)

`rea_ost::totals_job(&bid)` turns a bid into the input of `rea_core::ost_totals` (the rules
the parity harness checks against OST's stored totals). It reads the raw condition codes
from `condition_extra` (Type, Shape, Quantity1-3, UOM1-3, RoundQuantity, RoundUp, Backout,
DropRun, GridSize1-2, Gap) and the takeoff fields from `takeoff_raw` (the stored Position
numbers in real inches, ParentUID, BidZoneUID, BidTypAreaUID, TypGroupUID, Count,
GridOffsetX/Y, GridRotation).

What the job XML lacks compared with the job database (all 37 readable packages in JC's
corpus scanned 2026-10-20):
- No stored totals at all (no `BidTakeoffTotals`); the app always recomputes.
- `BidZones`, `BidTypAreas` and `BidTypAreaCounts` are present but empty in every package,
  and no takeoff without a condition (zone outline) appears. The importer reads `BidZone`
  (`UID`, `IsNegativeValues`), `BidTypAreaCount` (`BidAreaUID`, `BidTypAreaUID`, `Count`)
  and a takeoff with condition 0 and a zone as a zone outline, using the database column
  names; unconfirmed until a package with zones or typical areas turns up.
- `DropValue` and the condition `Grid` flag are read by no rule (drops use the takeoff
  `Count`; grid results use the grid sizes).

### BidAreas

The section was empty in the sample (an element named `BidAreas` inside `BidAreas`), so the
attribute names are unknown. The importer looks for descendant `BidArea` elements and reads
`UID`, `Name`, `ParentUID`. This is a guess to be fixed when a sample with areas exists.

### Alternates

`Bid/ALTERNATES/Bid` repeats the full bid structure with its own UIDs. Returned as
`OstImport.alternates`. The sample alternate had 13 pages, 17 conditions, 191 takeoffs.

## Open questions

1. Exact takeoff coordinate unit and origin (see above).
2. Full decoding of the `Quantity` and `UOM` result and unit codes, and `Shape` and `Pattern`.
3. Curved area outlines (area conditions with Curve 0 looked straight in the sample).
4. Area holes (backouts) and negative takeoffs: how they are stored.
5. Bid areas, zones, typicals: element and attribute names.
6. Whether any page rotation value other than 0 is stored in degrees as assumed.
7. `ScaleStyle` codes other than 1, and metric jobs (`MeasureBase` and scale units).
8. Whether `Index1` is 1-based for single-image (non-PDF) plan pages.
