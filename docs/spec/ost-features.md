# OST feature spec (Phase 0, clean-room)

Status: first draft, 2026-10-15.

This document describes what the reference takeoff product does, in our own words, so we
can build an equivalent feature set without copying it. It is a behavior spec, not a design.
Where we only know a feature exists but not its exact behavior, it is listed under Open
questions at the end.

## Sources and method

- Public user guide on the vendor help site (sections 01 to 20), read as article summaries.
- Public training video titles and descriptions (beginner tutorials, conditions and basic
  takeoff, multi-condition takeoff, collaboration).
- A short, partial look at the running app on JC's PC early in the session (main window,
  tabs, toolbars, left panel). JC then asked that study use docs and videos only, so the
  menu contents and the Summary, Estimate and Resources tabs were not observed directly.
- No binaries, installers, DLLs or project databases were decompiled, disassembled or read.
  No help text, icons or artwork were copied.

Confidence tags used below:
- [seen] observed in the running app.
- [docs] described in the public user guide or video descriptions.
- [inferred] our reasonable reading, needs confirmation by JC.

## 1. Overview

- Windows desktop application for on-screen quantity takeoff from plan images and PDFs. [seen]
- The installed product reports itself as version 4, Professional edition, even though the
  install folder name says 3. [seen]
- Main window has five top-level tabs: Projects, Takeoff, Summary, Estimate, Resources. [seen]
- Menu bar: File, Edit, View, Tools, Image, Project, Master. [seen] Contents not captured.
- Status bar shows current page number, page size and zoom level. [seen]
- Typical estimator flow: create project, load plans, set scale per page, define
  conditions, draw takeoff on pages, review totals on Summary, price on Estimate, export or
  print reports. [docs]

## 2. Storage model

- Projects are stored in a local database file that can hold many projects at once, for
  example one database per year or per estimator. [seen] The title bar shows the database
  name and a numeric project identifier. [seen, identifier meaning inferred]
- Databases can be created, opened and switched. [docs]
- Plan images are referenced from the project and stored alongside it. [inferred]
- Masters (section 15 below) live separately from projects so they can be reused. [docs]

Design note for us: we do not need to copy the multi-project single-file model. A project
folder (or one SQLite file per project) plus a shared library file is simpler. Decide at the
stack step.

## 3. Projects tab

- List of all projects in the open database, with basic job info (name, number, bid date,
  owner or GC, address, estimator). [docs, fields inferred]
- Create, copy, delete and archive projects. [docs]
- A project can carry a base bid plus alternates and change orders. Each alternate or change
  order holds its own takeoff that is reported separately from the base bid. [docs]
- Cover sheet for the project (job info block that prints with reports). [seen: cover sheet
  button on the quick toolbar]

## 4. Plans and pages

- Load plan sets from PDF and common image formats. Each sheet becomes a page. [docs]
- Pages carry a name and number, and can be auto-named from the title block by the AI
  helper. [docs]
- Pages can be organized into folders or groups in the left panel. [docs]
- Image tools: rotate, flip, adjust image, convert to a raster image format. [seen]
- Overlay: place one page over another (for example a revised sheet over the original) with
  color separation, to spot changes between revisions. [docs, seen: overlay command]
- Repeating pages: mark a page as typical so its takeoff counts multiple times (see
  Typicals). [docs]
- Hot links between pages (click a callout on one sheet to jump to a detail sheet), with an
  auto link helper that finds sheet references. [seen: hot link tool and auto link button]
- Named views: saved zoom and position on a page for quick return. [seen]

## 5. Scale

- Each page has its own scale. Takeoff on a page with no scale is blocked or flagged.
  [docs, enforcement inferred]
- Ways to set scale: [docs]
  - Pick from a list of standard architectural and engineering scales.
  - Calibrate from a known dimension: click two points, type the real length.
  - AI auto scale reads the scale note on the sheet.
- Verify scale: measure a known dimension after setting it to confirm. [docs]
- Separate horizontal and vertical scale for stretched scans. [inferred]
- Scale can be copied to other pages of the same size. [inferred]

## 6. Conditions

A condition is a named thing being measured, such as "3-5/8 in 20 ga stud wall, 10 ft" or
"ACT ceiling 2x2". Every takeoff shape belongs to exactly one condition. [docs]

### 6.1 Common properties [docs]

- Number and name, shown as two columns in the conditions list. [seen]
- Type: linear, area, count, or attachment.
- Appearance: color, line width, fill pattern or transparency, symbol for counts.
- Folder for grouping in the list. [seen]
- Layer assignment (see 6.7).
- Bid area default (optional).
- Notes or description.
- Quantity rounding settings.
- Cost code or other classification for reporting. [inferred from report names]

### 6.2 Linear conditions [docs]

- Measure length along polylines.
- Optional height: produces wall surface area (length times height). Option to count both
  sides of a wall.
- Optional thickness: with height, produces volume.
- Optional slope expressed as rise over run, which adjusts the true length.
- Drop and run adjustment: add a fixed vertical length at ends or segments, used for things
  like conduit drops. [docs, exact rule open]
- Results: length, wall area, volume, segment count, end count. [docs, list partly inferred]

### 6.3 Area conditions [docs]

- Measure closed polygons.
- Results: area, perimeter, and volume when thickness is set.
- Optional slope for sloped surfaces such as roofs, giving true surface area.
- Optional height: perimeter times height gives a wall area around the shape. [inferred]
- Grid pattern: overlay a tile grid aligned to the plan (for example 2x2 or 2x4 ceiling
  tile), so tile counts follow the drawn grid rather than raw area.
- Rounding of area per shape or per total.
- Snap the area outline to existing linear takeoff.
- Cutouts (backouts): holes inside an area that subtract from it.

### 6.4 Count conditions [docs]

- Click to place a symbol; each click is one each.
- Optional height or length per item, which enables totals such as cumulative height, area
  and volume (for example a column counted with a height).

### 6.5 Attachment conditions [docs]

- A count condition that must be placed on a parent shape.
- Linear attachments sit on linear parents (doors and windows on a wall).
- Area attachments sit on area parents (lights and diffusers in a ceiling).
- Option to deduct from the parent: a door can subtract its width from the wall length, or
  its area from the wall area.
- The attachment's own size (width, height or area) decides which deductions are possible
  and which parent types it fits.

### 6.6 Styles and style sets [docs]

- Style: a saved condition template with all its properties, reusable across projects.
- Style set: a named group of styles inserted into a project in one step (for example a
  standard drywall package of 20 conditions).
- Conditions can be copied, pasted, duplicated and deleted, including across projects.

### 6.7 Layers [docs, seen]

- Layers group conditions for display. The layers panel lists layers with a show toggle.
- Custom layers can be created and conditions assigned to them.
- Hiding a layer hides its takeoff on the page without affecting totals. [inferred]

## 7. Takeoff tools

### 7.1 Drawing [docs]

- Linear: click points to build a polyline; double click or a key finishes it. Arcs from
  three points and curve tools are available.
- Area: click points to build a polygon; closing returns to the first point.
- Count: single clicks.
- Manually set length: type a length for a segment instead of measuring it.
- Negative takeoff: a shape that subtracts from its condition total.
- Multi-condition takeoff: draw one shape that feeds several conditions at once (for
  example studs, track, board both sides and insulation along the same wall run).

### 7.2 Selecting and editing [docs, seen]

- Select tool, box select, select all of a condition.
- Move, copy, delete shapes; edit vertices; split and join lines. [docs, split and join
  inferred]
- Reassign selected shapes to a different condition.
- Undo and redo.

### 7.3 Navigation [seen]

- Page next and previous, zoom in and out, zoom to fit, magnifier, pan.

## 8. Automation

- Auto scale: reads the sheet's scale note. [docs]
- AI wall takeoff (sold under its own brand name by the vendor): runs on up to five pages at
  a time, finds walls, proposes linear takeoff, and can report both faces as wall surface
  area. Results come back for review: accept, edit, reassign to another condition, fine
  tune, rerun. [docs]
- Auto count: pick one symbol on the sheet; the tool finds matching symbols and places
  counts on them, with review before accepting. [docs, seen]
- Auto takeoff (non-AI fill): click inside a bounded region to trace its outline as an area.
  [seen: button exists, behavior inferred]
- Auto link: finds detail and sheet references and makes hot links. [seen: button exists,
  behavior inferred]
- Page auto naming from title blocks. [docs]

## 9. Typicals [docs]

- Typical area: a rectangular region with a multiplier grid. Takeoff inside the region is
  multiplied, but the extra copies are not drawn. Used for repeated units.
- Typical group: a frame of takeoff plus markers placed elsewhere. Each marker shows the
  takeoff physically and can carry its own occurrence count. Works with bid areas, so each
  marker can count into a different area.
- Repeating pages: a whole page counted N times, with a tool to assign virtual page copies
  to bid areas (for example floors 3 to 10 sharing one typical floor plan).
- The Summary tab shows how each of these multiplies the totals.

## 10. Bid areas and zones [docs, seen]

- Bid area: a label assigned to takeoff (building, floor, phase, unit type). Hierarchical,
  with sub-areas. A quick add helper creates many at once.
- The current bid area is chosen from a toolbar drop-down; new takeoff goes to it. [seen]
- Existing takeoff can be reassigned to another bid area.
- Zone: a drawn region on the page. Takeoff inside the region reports under that zone.
  [docs] Shapes crossing a boundary are probably split. [inferred]
- Difference: a bid area is an attribute you assign; a zone is geometric and computed.
  [docs]

## 11. Annotations and image legend [docs, seen]

- Markup tools that do not affect quantities: dimension, text, highlight, callout, arrow,
  line, rectangle, oval, polygon, revision cloud, freehand ink.
- Image legend: a legend on the page that lists the conditions shown, with color and
  quantity.
- Recorded legends: the legend can be stamped so it prints on the sheet. [docs]
- Comments and RFIs attached to locations on a page, listed in a Comments panel. [seen:
  panel tab; docs: RFI]

## 12. Left panel [seen]

- Tabs: project views (page tree), conditions, zones, comments, saved views.
- The conditions tab shows the tree of folders and conditions with quantity columns.
  [quantity columns inferred]

## 13. Summary tab [docs]

- Grid of cumulative totals per condition.
- Group and sort by bid area, zone, page and condition type, in any nesting order.
- Context menus to expand, collapse, jump to the takeoff, and export.
- Shows the effect of typical areas, typical groups and repeating pages.
- Prints as a summary report.

## 14. Estimate tab (unit cost worksheet) [docs]

- One row per condition total, with columns for labor, material and other unit costs.
- Extended cost per row is quantity times unit cost per column, then summed.
- Quantities can also be typed directly on this tab for items that are not drawn.
- Pricing options: markups, waste, tax, and price per unit or lump sum. [docs, exact options
  open]
- Can hand off to the separate bidding product for full pricing. [docs]

## 15. Masters [docs, seen]

- Central library of reusable items stored outside any one project: styles, style sets, bid
  area lists, typical setups, report settings. [docs, list partly inferred]
- The Master menu manages these. [seen]

## 16. Printing, reports and export [docs, seen]

- Print plans with takeoff and annotations, optionally with legend and cover sheet.
- Built-in reports seen by name: bid area, bids list, budget, condition set detail, cost
  code, takeoff, takeoff by area, worksheet, time card. [seen: names only]
- Export the Summary and Estimate grids to CSV and spreadsheet. [docs]
- Export a project package for sharing, and a separate bid package format for the bidding
  product. [docs]
- Import a project or bid package from another user. [docs]
- Email and cloud send from a quick toolbar. [seen]

## 17. Collaboration [docs]

- Share a project with another estimator through the vendor's cloud service and merge
  takeoff back. Out of scope for us until Phase 4.

## 18. Preferences [docs]

- Units (imperial and metric), default colors, rounding, autosave, display options.
  Specific options not captured.

## Open questions

1. Exact menu contents of Edit, View, Tools, Image, Project and Master.
2. Exact columns and grouping controls on the Summary and Estimate tabs.
3. Exact rounding rules: per shape, per page, or per total, and the rounding increment.
4. Drop and run rules for linear conditions.
5. How zones split shapes that cross a boundary, and whether counts are split.
6. Which report fields JC actually uses. Best answered by the reference job exports in
   `reference/`.
7. Whether a "both sides" wall setting doubles area only or also doubles stud and track.

These are best closed by JC exporting the Summary and Estimate grids of one real job to
Excel, so we can match quantities against them.

## Phase 1 MVP mapping

Phase 1 must deliver, from this spec:
- Projects: create, open, basic job info. (section 3, no alternates yet)
- Plans: load PDF, page list, page names, zoom and pan. (sections 4 and 7.3)
- Scale: standard list plus calibrate from known length, per page. (section 5)
- Conditions: linear with height and both sides, area with perimeter and cutouts, count.
  Color, folder, name, number. (6.1 to 6.4)
- Takeoff: draw, select, move, delete, vertex edit, undo, reassign. (7.1, 7.2)
- Bid areas: flat list, toolbar picker, reassign. (section 10, no zones yet)
- Summary: totals per condition grouped by bid area and page. (section 13)
- Export: Summary grid to Excel. (section 16)

Deferred to Phase 2: attachments, styles and style sets, layers, typicals, zones, overlay,
annotations, legends, hot links, automation, alternates and change orders, printing.
Deferred to Phase 3: Estimate tab and pricing, together with the bidding module.
