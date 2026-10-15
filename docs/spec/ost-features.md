# OST feature spec (Phase 0, clean-room)

Status: second draft, 2026-10-16. Rebuilt from a full read of the public On-Screen Takeoff
user guide (all 20 categories, 286 articles). Replaces the 2026-10-15 first draft, which was
built from article summaries only.

This document describes what the reference takeoff product does, in our own words, so we
can build an equivalent feature set without copying it. It is a behavior spec, not a design.

## Sources and method

- Public user guide on the vendor help site, every article in categories 01 to 20, read in
  full and paraphrased into the evidence ledger `docs/study/ost/` (one file per group of
  categories). Every bullet there has a ledger ID such as `OST-05-015`. This spec cites those
  IDs so any rule can be traced back to the article that supports it.
- A short, partial look at the running app on JC's PC on 2026-10-15 (main window, tabs,
  toolbars, left panel). No further PC study.
- Not used, by rule (CLAUDE.md section 5): binaries, installers, DLLs, project databases,
  decompilers, disassemblers, debuggers, or any reverse-engineering tool pointed at the
  product. No help text, icons or artwork copied. Raw article text is not committed.

Confidence tags:
- [docs] stated in the guide.
- [inferred] our reading of the guide, needs confirmation against a real export.
- [unknown] the guide does not pin it down. Listed in the Unknowns register at the end.

Method (clean-room evidence loop):
1. Ledger: every behavior gets a ledger ID and a source.
2. Unknowns register: every gap gets an entry and a plan to close it.
3. Black-box verification: the same plan PDF is taken off in OST and in our app, and the
   exported quantities are compared line by line. A mismatch reopens the ledger entry.
4. Coverage: percent of ledger entries built and verified, reported per phase.

## 1. Object model

- Database holds projects, master tables and links to plan files. Plans are never stored in
  the database, only paths. [docs] OST-20-001, OST-03-009
- Project (also called bid or job) = one Base Bid plus child bids: Alternates (pre-contract
  options) and Change Orders (post-contract changes). [docs] OST-03-001, OST-03-026
- Child bids share the plan pages, Bid Areas and Typical Areas of the base bid. Plan
  organization happens only on the base. [docs] OST-03-026
- Each bid has its own Conditions, takeoff, annotations, comments and pricing. Deleting a
  Condition in the base does not delete it in a child and vice versa. [docs] OST-04-048,
  OST-17-019
- Page = container for one plan image (plus an optional overlay), its scale, its takeoff,
  annotations and comments. Page Index = row position. Sheet No. and Sheet Name are free
  text taken from the title block. [docs] OST-01-046, OST-11-004
- Condition = definition of one measurable building item (name, style, dimensions, layer,
  type, up to three result quantities, appearance). One Condition per unique item; a height
  change needs its own Condition. [docs] OST-05-001
- Takeoff object = a shape drawn with a Condition on a page (linear, area, count,
  attachment, typical group marker, zone).
- Every bid has an origin ID that persists through export and import and is used to match
  an incoming package to an existing bid. [docs] OST-16-045

## 2. Units, measurement entry, rounding

- Units: imperial EA, IN, LF, LY, IN2, SF, SY, SQ (100 SF), CF, CY; metric EA, mm, m, mm2,
  m2, mm3, m3. [docs] OST-01-034
- Measure mode (imperial or metric) is set per database; each bid has a Base Measurement
  (inches or millimeters) copied from the database default. Do not mix in one database.
  [docs] OST-01-035, OST-20-006
- Imperial entry in any dimension box: the last two digits are inches, the digits before
  are feet. `800` = 8'0", `1010` = 10'10", `824` = 8'24" = 10'0". Inches over 11 carry.
  total_in = feet * 12 + inches. Decimals allowed for fractions of an inch. Metric entry is
  always millimeters. [docs] OST-01-038 to OST-01-040
- Stored values are full precision. Rounding on screen is display only: default 0
  decimals, per result column setting 0..n or Floating (up to 6 digits). The Summary
  setting also drives the Conditions list and area labels; the Estimate tab has its own.
  Program level, not per bid. [docs] OST-01-041 to OST-01-044, OST-12-008, OST-13-008
- Condition rounding (separate from display): an optional per-Condition "Round Quantity"
  rounds EACH drawn segment UP to the next multiple of an increment; the drawn shape is
  unchanged and the final total is not rounded again. Example: a 121 in square at a 6 in
  increment computes as 126 x 126 in = 110.25 SF instead of 101.67 SF. [docs] OST-05-016,
  OST-05-026, OST-05-027
- A bid preference "Takeoff in Increments" (default 1 in or 10 mm) sets the smallest unit of
  measurement. [unknown] U-02

## 3. Projects tab and project lifecycle

- Grid of all projects across open databases, child bids nested under their base. Columns
  include number, name, estimator, job no., status, bid date, copy-from, copy timestamp,
  flags, notes. Bid date color: past black, future blue, today red. [docs] OST-02-001,
  OST-01-074
- Four persistent filters (name and job no. substring, estimator, job status). Group by job
  status. Folders. [docs] OST-02-006, OST-02-007
- Create from scratch, duplicate, import, or planroom file. Duplicate copies pages,
  Conditions, takeoff and name; only the bid number changes. [docs] OST-03-002, OST-03-024
- Bid numbers come from a "next bid number" counter; a number is consumed even when the
  create is cancelled. [docs] OST-01-026
- Delete base bid moves it and its children to a Deleted Bids folder (restorable). Delete a
  child bid is permanent. Emptying the folder is permanent. [docs] OST-03-025
- Job statuses can carry a Lock flag that makes the project read-only for everyone. [docs]
  OST-18-009
- Template projects: a normal project kept as a starting point; it carries Conditions,
  layers, bid areas, typicals, legend settings AND Estimate tab pricing (Styles do not carry
  pricing). [docs] OST-03-022

## 4. Cover sheet and plan organizer

- General fields: job status, bid no. (unique), project name (required, 75 chars, plain
  characters), job no. (50), estimator, notes, Price Using (Estimate tab, Excel, bidding
  app), Typicals mode (Typical Groups or Typical Areas, mutually exclusive), bid date and
  time, Areas and Typical Areas buttons. [docs] OST-03-007
- Preferences tab: base measurement, takeoff increment, new page size and scale defaults,
  image legend content and sort. Defaults copied from the database New Project options at
  creation. [docs] OST-03-008, OST-19-010 to OST-19-013
- Plan organizer grid columns: page index, sheet no., sheet name, repeats, page size,
  scale, image file, index within a multipage file, overlay image, show (original, overlay,
  both), status. [docs] OST-03-010
- Add blank page, add single file, auto-add all files of one type in a folder (no
  subfolders), add folder, duplicate page (no takeoff copied), delete page, move up/down,
  drag to reorder, page folders up to any depth (50 char names). [docs] OST-03-011,
  OST-03-016, OST-03-017, OST-11-015, OST-11-016
- A multipage PDF becomes one page per PDF page. [docs] OST-03-016
- Image paths are relative to the bid's Image Folder or explicit full paths. Missing files
  show red; Change Folder rewrites the path prefix of selected pages; a missing-image
  assistant can relink every page from the same original folder. [docs] OST-11-019 to
  OST-11-029
- Deleting a page deletes it from the base and all child bids with all takeoff, after a
  confirmation that cannot be turned off, and cannot be undone. [docs] OST-03-012,
  OST-04-012

## 5. Plans: viewing and image adjustment

- Navigation: page dropdown (blue icon = page has takeoff, green check = marked complete),
  previous/next (Page Up/Down), back/forward view history, go to page (Ctrl+G). [docs]
  OST-04-016 to OST-04-019, OST-04-030
- Zoom: full page (*), in (+), out (-), 100% (/), custom, Ctrl+wheel, zoom box, magnifier
  loupe. Pan: wheel, Shift+wheel horizontal, right-drag, Alt, pan window thumbnail. [docs]
  OST-01-019, OST-01-021, OST-04-021, OST-04-022, OST-04-042
- "Follow the leader": after two consecutive pages get the same zoom-to-fit or rotation, the
  app offers to apply it to the rest. [docs] OST-04-028, OST-11-038
- Per-page display settings stored in the database, never written to the file: rotate 90,
  flip, deskew (draw a line along a known horizontal), enhance (on by default), darken,
  invert, bitonal; apply to one page or all. Rotating a page rotates its takeoff. [docs]
  OST-11-030 to OST-11-041
- Find Text across all pages of a PDF text layer, hit count per page, highlights. [docs]
  OST-01-077
- Optional TIFF conversion for speed (copy in a Converted folder). Low priority for us.
  [docs] OST-11-042 to OST-11-048
- Second and third view windows: annotate and set scale, no takeoff. [docs] OST-04-052 to
  OST-04-054
- Overlays: one single-page overlay per page, shown greyscale; content only on the overlay
  shows blue, only on the base shows red; two-point resize, align (move and rotate
  handles), deskew; overlay edits write a copy of the file. [docs] OST-11-049 to OST-11-063

## 6. Scale

- One scale per page, set before takeoff and then verified. Styles: architectural, civil,
  metric; preset list or custom; "apply to all pages". [docs] OST-06-003, OST-06-012
- Calculate scale: type a known length, drag across it on the plan; scale = real length /
  drawn length. The label shows the length rounded up to an inch but the math uses the
  exact value. [docs] OST-06-015 to OST-06-018
- Verify with the dimension tool horizontally and vertically. [docs] OST-06-019
- Different horizontal and vertical scales are not supported; multiple scales on one sheet
  are handled by duplicating the page. [docs] OST-06-004, OST-06-005
- Changing scale after takeoff exists is discouraged and blocked when a child bid has
  takeoff on that page. [docs] OST-06-006, OST-06-014
- New pages default to size E (30 x 42 in), architectural, 1/8" = 1'-0". [docs] OST-19-011

## 7. Conditions

### 7.1 Common properties
- Styles: Linear, Area, Count, Attachment, plus Typical Group. Name up to 255 chars, Type
  (grouping tag, blank shows as Unassigned), Layer (default "Default"), Condition No. (auto,
  editable, unique), color, pattern, notes. [docs] OST-05-002, OST-05-012, OST-18-011
- Up to three result quantities per Condition (Q1 required). Q2 and Q3 only show when Q1 is
  valid. [docs] OST-05-014, OST-05-039
- OST applies no waste factor to any result; waste is applied downstream in bidding. [docs]
  OST-05-018, OST-05-020
- Ways to add: new, duplicate (new number and color), from Style Library, insert Style Set,
  copy/paste from another bid (also created automatically when takeoff is pasted). [docs]
  OST-05-004, OST-05-013, OST-05-047
- Deleting a Condition deletes all its takeoff on all pages of that bid, after a
  confirmation that cannot be turned off, with no undo. [docs] OST-05-049

### 7.2 Linear results
Symbols: L = total drawn length, H = height, T = thickness.
- Length = L. Segment Count = number of segments (a square = 4).
- Surface area: one side H*L; both sides 2*H*L; top or bottom T*L; top and bottom 2*T*L;
  one end H*T; both ends 2*H*T; all sides (duct) 2*T*L + 2*H*L.
- Volume = H*T*L. [docs] OST-05-015
- Slope (rise:run): drawn line is the plan projection; true length = L * sqrt(1 +
  (rise/run)^2). 12 ft at 4:12 = 12.649 LF. [docs] OST-05-019
- Advanced: Round Quantity per segment; Drop/Run (an extra length added to each segment,
  prefilled from a default, editable per segment); Manual Length Entry (length typed, not to
  scale); Connect to perpendicular linears within a tolerance; Trim at a perpendicular;
  curved segments by default. [docs] OST-05-016, OST-06-082 to OST-06-086
- How drop/run feeds area and volume, and whether end areas multiply by segment count:
  [unknown] U-03, U-04

### 7.3 Area results
Symbols: A = area, BA/BL/BV = backout area/perimeter/volume, AA/AL/AV = attachment
area/perimeter/volume, GL = visible grid length.
- Area = A - BA. Area ignoring backouts = A. Area minus attachments = A - BA - AA.
- Perimeter = outer perimeter plus backout perimeters, excluding attachment perimeters;
  variants without backout perimeters and plus attachment perimeters. [docs, sign
  convention unknown] OST-05-023, U-05
- Grid length (visible) = GL. Tile count (average) = area / tile area. Tile count (visible) =
  every whole or partial tile shown by the aligned grid. Area count = number of area
  objects.
- Volume = A * T - BV; variants ignoring backouts and minus attachments. [docs]
  OST-05-023
- Slope for areas: rise:run on plan, run:rise on elevation; only area and volume are
  slope-adjusted; factor = sqrt(1 + (rise/run)^2). [docs] OST-05-022, OST-05-031
- Advanced: Round Quantity per segment, grid (tile size, gap, alignment), show dimension
  label (Q1 at the object center), show name label (9 chars), snap to linear (inner,
  center, outer edge, tolerance), auto-fill a room enclosed by connected linears. [docs]
  OST-05-024, OST-06-091

### 7.4 Count results
Symbols: C = count, H, W, D = height, width, depth (D = W except the rectangle shape).
- Count = C; total height = C*H; perimeter = C*2*(W+D); top or bottom = C*W*D; top and
  bottom = 2*C*W*D; one width side = C*H*W; both = 2*C*H*W; one depth side = C*H*D; both =
  2*C*H*D; all sides = 2*C*(H*W + H*D); all sides plus top and bottom = 2*C*(H*W + H*D +
  W*D); volume = C*H*W*D. [docs] OST-05-035
- Shapes: square, circle, triangle, diamond, ellipse, hexagon, octagon, rectangle, and
  opening shapes (right/left hand door, double door, double egress, three window types).
  The shape decides which results are offered. [docs] OST-05-032, OST-05-036, OST-05-037
- Count size is set on the Condition only; all objects of a Condition share it. Display
  size percent scales the symbol on screen only. [docs] OST-05-034, OST-09-010
- Formulas for round shapes appear to be bounding-box style. [unknown] U-06

### 7.5 Attachments
- A count-like object that lives on a parent linear (doors, windows, openings) or area
  (lights, vents) and moves with it. Linear shapes attach only to linears, area shapes only
  to areas. [docs] OST-05-040
- "Backout quantities from parent" (default on) subtracts the attachment's Q1 from the
  parent. Snap to the parent's grid; grid orientation must match. [docs] OST-05-042,
  OST-05-025
- Fit rules: width must fit along the segment; must sit fully inside a parent area; not on
  curved segments. [docs] OST-06-107, OST-06-108
- Exactly which parent results are reduced by which attachment result. [unknown] U-07

### 7.6 Layers, styles, style sets
- Layers are show/hide groups and do not change quantities. System layers: Image,
  Annotation, Comments, Default. Hidden layer = not printed, and its Conditions are greyed
  and unusable. A layer in use cannot be deleted. [docs] OST-05-005 to OST-05-010
- Style = Condition saved to the master library (no link back; editing either never
  changes the other). Style Set = named group of styles with an owner; inserting a set
  creates one Condition per style. [docs] OST-05-050 to OST-05-054, OST-18-012 to
  OST-18-014

## 8. Drawing takeoff

- Space toggles Takeoff and Select. Snap angle default 15 degrees with a right-angle
  indicator; Shift disables snap while drawing. Separate snap values for mouse and
  digitizer. [docs] OST-01-018, OST-06-072, OST-19-006
- Linear: press-drag-release for one segment; continuous mode (click each point,
  double-click to end); freehand contour converted to short segments by an "intelligent
  curve" density setting; arc (bend one segment by its middle handle); three-point curve;
  set length on a segment. [docs] OST-06-073 to OST-06-086
- Area: rectangle drag; polygon (click corners, double-click the last, auto close); invalid
  self-crossing points are rejected with a sound, Esc steps back one point; auto-fill inside
  connected linears; add or remove vertices. [docs] OST-06-089 to OST-06-094
- Backouts: draw a hole inside one selected area; the hole is its own object (move, resize,
  copy, paste) and can be filled with another area Condition. [docs] OST-06-098 to
  OST-06-100, OST-09-032
- Grid: align by dragging a position handle and a rotation handle; alignment changes the
  visible tile count, not the average. [docs] OST-06-096, OST-06-097
- Count: click each location. Auto Count (cloud AI, up to 20 pages per run) and a legacy
  local pixel-match tool. [docs] OST-06-101 to OST-06-105
- Multi-condition takeoff: pick several Conditions of mixed styles and draw once. Any area
  present means area drawing; else linear; else count. Counts land on vertices or segment
  breaks. [docs] OST-06-065 to OST-06-070
- Negative takeoff: mark objects "count as negative" or draw them inside a negative zone;
  total = sum(positive) - sum(negative). [docs] OST-06-109 to OST-06-111
- Manual quantities: typed on the Estimate tab for a Condition with no drawn takeoff (Q1
  only, shown blue); drawing any takeoff later replaces it for good. [docs] OST-06-112 to
  OST-06-114
- Mark page "takeoff complete". [docs] OST-04-030

## 9. Selecting and editing

- Click select, Shift+click to add or toggle, rope select in "touching" (default) or
  "inclusive" mode. Select all objects of chosen Conditions on the page. [docs] OST-09-001
  to OST-09-003
- Undo and redo for takeoff and annotation only, cleared when the page changes. [docs]
  OST-09-005, OST-01-056
- Resize by handles; area vertices are shared so a move drags both neighbors. [docs]
  OST-09-008, OST-09-009
- Split a linear at clicked points; split an area with a line or a polygon cut; simplify an
  area (remove extra vertices); move one endpoint keeping the other fixed. [docs] OST-09-015
  to OST-09-024
- Copy and paste within a page, across pages and across bids (missing Conditions created).
  Repeated paste offsets down-right; optional "intelligent paste" keeps the user's offset and
  snaps to alignment. Arrow keys nudge. Attachments only copy with their parent. [docs]
  OST-09-025 to OST-09-034
- Rotate selection about a shared center (snap angle, Ctrl+R); spin each object about its
  own center (Ctrl+S); blocked if the result leaves the page. [docs] OST-09-035,
  OST-09-036, OST-04-037
- Reassign selected objects to another Condition of the same style (page scoped); duplicate
  a Condition and move this page's takeoff to the copy. [docs] OST-09-039 to OST-09-044

## 10. Bid areas and zones

- Bid areas: named list up to three levels deep (building, floor, room), 50 char names,
  quick add a numbered range. Each object belongs to exactly one bid area or typical area,
  or none. Assign after drawing ("assign to current area") or automatically while drawing.
  Non-active areas draw greyed. An area in use cannot be deleted. [docs] OST-07-001 to
  OST-07-014
- Zones: drawn polygons that may touch but not overlap or nest. Linear and area objects are
  split geometrically across zones (a 20 ft wall half in each zone reports 10 ft per zone on
  the Summary tab while the Conditions list still shows 20 ft). Counts and attachments
  belong to a zone when at least half their box is inside. A zone can be negative. Parts
  outside every zone are Unassigned. [docs] OST-07-015 to OST-07-024
- Exact clipping math for areas, surface areas and volumes at zone borders. [unknown] U-08

## 11. Typicals

Three methods; Typical Areas cannot be combined with the other two in one bid. [docs]
OST-10-001 to OST-10-005
- Typical Areas: a grid of counts (typical area by bid area). Take off one instance and
  assign it to the typical area; results are multiplied by the count in each bid area.
  [docs] OST-10-007 to OST-10-018
  - result(condition, bid area) = direct takeoff + sum over typical areas of (single
    instance qty * count in that bid area). [inferred] OST-10-010
- Typical Groups: a Condition-like group with a frame drawn around takeoff and markers
  placed elsewhere. Takeoff inside the frame is excluded from totals until markers exist;
  each marker adds one copy of the frame content; a marker count can be set to N. Only the
  marker's bid area counts. Child bids receive empty copies of frames and markers so
  variances (including negative takeoff) apply to every marker. [docs] OST-10-019 to
  OST-10-047
  - total = takeoff outside frames + sum over markers (frame qty * marker count). [inferred]
    OST-10-026
  - Frame membership: linear and area objects touching or inside; counts and area
    attachments by center point; linear attachments by opening center. [docs] OST-10-028
    to OST-10-030
- Repeating pages: a Repeats count per page creates virtual copies. The typical page itself
  is not counted; each virtual page counts once and maps the typical page's bid areas to
  its own bid areas. Repeats can only be lowered by deleting virtual pages. [docs]
  OST-10-048 to OST-10-062
- Rounding interaction of all three. [unknown] U-09

## 12. Annotations, legend, links, comments

- Annotations never count as takeoff and always live on the Annotation layer: dimension
  line (two clicks, breaks where it crosses linear takeoff), text box, highlighter (48
  colors), callout, arrow, line, rectangle, oval, polygon, cloud, freehand ink. Defaults per
  type in program options, not retroactive. [docs] OST-08-001 to OST-08-043
- Image Legend: one switch for all pages; lists each Condition used on the page (visible
  layers only) with swatch, number, name, optional type and up to three page quantities;
  sort by number, name or type; grouped as one block or ungrouped into movable labels.
  Printing filters to the active bid area by default. [docs] OST-08-044 to OST-08-060
- Named views (named page regions) and hot links (click target that opens exactly one named
  view, by default in the second window). Deleting a view deletes its links. Hot links are
  selected only by roping. [docs] OST-08-030 to OST-08-038
- Comments: threaded, per page and per bid, author 50 chars, text 250 chars, icon color
  used as status. [docs] OST-17-018 to OST-17-030

## 13. Summary tab

- Per Condition rows with its result columns (Q1 to Q3). Group by page, type, zone and bid
  area in any combination; nesting order is page, then type, then zone, then area; group
  headers sort ascending (areas keep their dialog order); column sort applies inside groups.
  Default grouping is by area. [docs] OST-12-001 to OST-12-009
- Options show typical area and typical group single-instance and cumulative totals;
  repeating pages roll into a multi-area total. [docs] OST-12-012 to OST-12-018
- Delete a Condition from here (not while grouped). Copy rows to a spreadsheet; print
  preview report honoring grouping and sort; export to CSV (same grouping, no header row).
  [docs] OST-12-005, OST-12-011, OST-16-009, OST-16-035

## 14. Estimate tab (rough unit cost)

- Every Condition in the bid, used or not, with input columns for material, labor and
  subcontractor unit prices and a computed total based on Q1. Hide zero rows. Group by page
  and type. One set of prices per Condition even when grouped. [docs] OST-13-001 to
  OST-13-010
- total = Q1 * (material + labor + sub). [inferred] OST-13-002, U-10
- Pricing is kept by Template projects, not by Styles. [docs] OST-13-006

## 15. Pricing hand-off

- Price Using is chosen per project: Estimate tab (default), Excel, or the bidding app.
  [docs] OST-14-001, OST-14-002
- Excel: each Condition result links to a cell or a named range; F9 pushes cumulative bid
  totals (no breakdown); one workbook per project. [docs] OST-14-004 to OST-14-008
- Bidding app link (interactive or manual on F9): OST owns projects, child bids, Conditions,
  bid areas, typical areas and quantities; the bidding app owns items, labor, material and
  all other costs. One takeoff database to one bidding database. Shared lookup tables:
  employees, job statuses, condition types, cost codes, payroll classes. [docs] OST-14-009 to
  OST-14-016
- Zones become top-level bid areas on the bidding side. [docs] OST-07-019

## 16. Printing and reports

- Takeoff print preview: range current view (default each time), current page, pages with
  takeoff, all pages; size fit to page or 100% (tiles across sheets); crop to chosen sheets;
  footer with page, scale percent, project, database, date and time. Hidden layers and
  non-active bid areas do not print. Print Area tool prints a chosen frame scaled to one
  sheet. [docs] OST-15-001 to OST-15-014
- Tab reports (projects, summary, estimate) via print preview; export to PDF, Excel, Word.
  [docs] OST-16-006, OST-16-007

## 17. Import, export, collaboration

- Two bid files: a package with optional images and comments that matches and can
  overwrite the origin bid, and a data-only takeoff file that always imports as a new bid
  and feeds the bidding app. Newer files cannot be imported into older versions. Missing
  master records are created on import. We define our own formats. [docs] OST-16-012 to
  OST-16-046
- Package page range: all, pages with takeoff, pages with takeoff and comments, custom.
  Importing a partial package over an existing bid deletes the missing pages. [docs]
  OST-16-021, OST-16-024
- Cloud project sharing (upload a package, visible to the organization, download into a
  chosen database). Phase 4 for us. [docs] OST-17-031 to OST-17-035

## 18. Masters, options, databases, security

- Masters: employees (CSV import), job statuses (with lock flag), condition types, style
  library and sets, default layers. Referenced records cannot be deleted. Master sync copies
  and updates master rows from one database to another, never projects. [docs] OST-18-001
  to OST-18-017, OST-20-013
- Program options are per Windows user; New Project options are per database and apply to
  bids created afterwards. [docs] OST-19-001
- Databases: single-user local file or shared server. Automatic backup and compact on close.
  One editor per project at a time in a shared database. [docs] OST-20-002 to OST-20-022
- Security: optional login per database; access levels with Full / Read Only / No Access on
  four areas (others' bids, projects list, masters, styles). Bid ownership is the estimator
  on the cover sheet. [docs] OST-20-016 to OST-20-021

## 19. Cloud AI features (out of scope for Phase 1 and 2)

Auto Scale, Auto Takeoff, Auto Count, Auto Link, Auto Name all upload plans to a vendor
cloud service. We may build our own later; the guide shows the expected outputs (Conditions
in an "Auto Takeoff" folder with auto layers and types). [docs] OST-01-052, OST-06-023 to
OST-06-064, OST-08-024 to OST-08-029

## Unknowns register

Each unknown is closed by black-box comparison: draw the case in OST and in our app, export
both, compare. Inputs come from JC's reference exports in `reference/` unless noted.

- U-01 Measurement box parsing of apostrophes, quotes, fractions and decimal feet.
  (OST-01-038) Close by typing test values.
- U-02 What "Takeoff in Increments" rounds (vertex snap, segment, or total). (OST-03-008,
  OST-19-010)
- U-03 Drop/run effect on area, volume and segment count. (OST-05-017, OST-06-084)
- U-04 Whether linear end areas multiply by segment count or object count. (OST-05-015)
- U-05 Sign convention of backout and attachment perimeters. (OST-05-023)
- U-06 Count formulas for round and polygon shapes. (OST-05-035)
- U-07 Which parent results an attachment reduces (wall SF, length, both). (OST-05-043,
  OST-06-108)
- U-08 Zone clipping of areas, surface areas and volumes. (OST-07-024)
- U-09 Typicals math per result type, with negative takeoff and rounding. (OST-10-010,
  OST-12-015)
- U-10 Estimate tab total formula and rounding. (OST-13-002)
- U-11 Arc and three-point curve length formulas. (OST-06-080)
- U-12 Rescale behavior for existing takeoff. (OST-06-006)
- U-13 Simplify-area tolerance, split-area inheritance of backouts and attachments.
  (OST-09-020, OST-09-024)
- U-14 Overlay two-point resize math (uniform or not, rotation). (OST-11-059)
- U-15 CSV export column order and number format. (OST-16-011)
- U-16 Delete Condition from the Estimate tab: current bid or whole project. (OST-13-005)

## Phase 1 MVP mapping

Phase 1 delivers, from this spec:
- Projects: create, open, duplicate, basic cover sheet. (sections 3, 4; no child bids yet)
- Plans: load PDF, multipage split, page list with sheet no. and name, zoom, pan, page
  navigation. (sections 4, 5)
- Units and entry: imperial and metric, packed feet-inches entry, display decimals.
  (section 2)
- Scale: presets plus calculate from a known length, per page, verify with a dimension
  line. (section 6)
- Conditions: linear (length, segment count, all surface areas, volume, slope), area (area,
  perimeter, volume, backouts, slope), count (count and shape results). Up to three
  results. Name, number, type, color, layer. (sections 7.1 to 7.4)
- Takeoff: linear (drag and continuous), area (rectangle and polygon), backouts, count,
  select, move, resize, vertex edit, delete, undo, reassign. (sections 8, 9)
- Bid areas: flat list, active area, assign. (section 10, no zones yet)
- Summary: totals per Condition grouped by page, type and area. (section 13)
- Export: Summary to Excel. (section 13)

Phase 2: attachments, rounding per segment, grids and tile counts, drop/run, curves,
layers, styles and style sets, zones, typicals, alternates and change orders, annotations,
legend, named views and hot links, comments, overlays, printing, packages, masters,
security.
Phase 3: Estimate tab and pricing, together with the bidding module (see
`quickbid-features.md`).
Phase 4: company systems, cloud sharing, AI automation.
