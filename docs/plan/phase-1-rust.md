# Phase 1 build plan: takeoff MVP in Rust

Status: draft for JC review, 2026-10-16.
Scope source: "Phase 1 MVP mapping" in `docs/spec/ost-features.md`.
Toolchain on JC's PC: Rust 1.94.0, stable-x86_64-pc-windows-msvc.

## 0. End goal

Functional one-to-one parity with OST and QuickBid: every ledger behavior built, the same
workflow (tabs, panels, tools, shortcuts, feet-inch entry), and quantities and bid totals
that match their exports for the same job. Not copied: code, icons, artwork, help text,
names, logos, or pixel-exact screens (clean-room rules, CLAUDE.md section 5). Phase 1 is
the first slice of that target, not a reduced product.

## 1. Goal and definition of done

A Windows desktop app where JC can:
1. Create a project and load a multipage plan PDF.
2. Set and verify scale per page.
3. Create linear, area and count Conditions with up to three results.
4. Draw, edit and delete takeoff; assign it to bid areas.
5. See totals per Condition grouped by page, type and area.
6. Export the Summary to Excel.

Phase 1 is done when:
- Every Phase 1 ledger item listed in section 7 has a passing test.
- One real job from `reference/` is taken off in our app, and the Summary quantities match
  the OST export for the same job within rounding (CLAUDE.md section 6).
- There is a one-click launcher, a one-page guide, and a check on JC's smallest screen
  (CLAUDE.md section 7).

Out of scope for Phase 1: attachments, per-segment rounding, grids and tile counts,
drop/run, curves, zones, typicals, alternates and change orders, annotations, legend,
overlays, printing, packages, the Estimate tab and all bidding. These are Phase 2 and 3, and
the data model leaves room for them.

## 2. Architecture

One Cargo workspace, four library crates plus the app. The math has no UI dependency, so
it is testable on its own and the bidding module (Phase 3) can reuse it.

```
app/                         (workspace root inside this repo)
  crates/
    core/      geometry, units, scale, conditions, quantity math, summary. No I/O.
    store/     project file (SQLite), load/save, schema migrations.
    pdf/       PDF page count, page size, rendering to tiles (wraps the PDF engine).
    export/    Summary to .xlsx.
  desktop/     desktop UI: viewer, tools, panels, undo stack. Uses the four crates.
  tests/       golden tests driven by fixture and reference jobs.
```

### Crate choices (pick exact versions at scaffold time, pin them in Cargo.lock)
- **UI: `eframe` / `egui`.** Immediate-mode, pure Rust, runs natively on Windows, good
  canvas painting for takeoff shapes, and `egui_extras` tables for the Summary grid.
  Fastest path to a working tool. Alternatives considered: Tauri (web UI, two languages),
  Slint and Iced (fewer ready-made widgets for grids). Revisit only if egui tables fall
  short in Phase 2.
- **PDF rendering: `pdfium-render`** with a bundled `pdfium.dll` (the engine Chrome uses,
  permissive license). Most accurate on construction plan sets with heavy vector content.
  Avoid MuPDF bindings (AGPL). A pure-Rust renderer can be tried later as an option.
- **Geometry: `geo`** for polygon area, perimeter, point-in-polygon and, in Phase 2, zone
  clipping. Our own small types for drawn segments.
- **Storage: `rusqlite`** (bundled SQLite). One `.reaproj` file per project. Plans are
  linked by path, never copied in, matching the reference behavior (OST-03-009).
- **Excel: `rust_xlsxwriter`.**
- **Serialization: `serde`. Errors: `thiserror`** in libraries, **`anyhow`** in the app.
  **Logging: `tracing`.**
- **Tests: built-in `cargo test`, plus `insta`** for snapshot tests of Summary output.

## 3. Data model (Phase 1 subset, Phase 2 ready)

All lengths are stored as `f64` inches (imperial projects) or millimeters (metric
projects) at full precision. Rounding is display only (OST-01-041).

- `Project { id, name, bid_no, job_no, estimator, notes, measure: Imperial|Metric,
  created, origin_id }`
- `Page { id, project_id, index, sheet_no, sheet_name, file_path, file_page_index,
  width, height, scale: Option<Scale>, rotation, complete }`
- `Scale { real_per_page_unit, style: Architectural|Civil|Metric|Custom, label }`
  (one scale per page, OST-06-003)
- `Condition { id, project_id, number, name, style: Linear|Area|Count, ctype, layer,
  color, pattern, height, thickness, width, depth, slope: Option<(rise, run)>,
  count_shape, results: [Option<ResultKind>; 3] }`
- `ResultKind` = the result list in spec sections 7.2 to 7.4 (length, segment count,
  surface area variants, volume, area, perimeter, area count, count shape results).
- `Takeoff { id, page_id, condition_id, bid_area_id: Option, negative, geometry }` where
  `geometry` = `Polyline(points)` | `Polygon { outer, backouts }` | `Point(point)`. Points
  are stored in page units (PDF points), so a scale change recomputes quantities without
  moving shapes.
- `BidArea { id, project_id, name, parent_id, order }` (flat in Phase 1; `parent_id` kept
  for the three-level tree in Phase 2).

Room left for Phase 2 without breaking migrations: attachments (parent takeoff id), zones
(own table), typical areas (grid table), child bids (`bid_id` on Condition and Takeoff,
defaulting to the base bid).

## 4. Core math (crate `core`)

Every formula comes from the spec with its ledger ID, and each gets a unit test built from
the spec's own worked example.

- **Packed feet-inch entry parser** (OST-01-038): `800` = 96 in, `824` = 120 in, `1010` =
  130 in; metric input = millimeters.
- **Scale:** presets per style; calculate from a known length (OST-06-015 to OST-06-018).
- **Linear results** (OST-05-015): length, segment count, the six surface areas, volume.
  Slope factor sqrt(1 + (rise/run)^2); test 12 ft at 4:12 = 12.649 LF (OST-05-019).
- **Area results** (OST-05-023): area minus backouts, area ignoring backouts, perimeter
  variants, volume minus backout volume, area count. Slope applies to area and volume only
  (OST-05-022).
- **Count results** (OST-05-035): count, total height, perimeter, surface areas, volume by
  shape.
- **Negative takeoff:** total = sum(positive) - sum(negative) (OST-06-110).
- **Summary:** totals per Condition, grouped by any of page, type and area in the fixed
  nesting order page, then type, then area (OST-12-009). Unassigned is its own group.
- **Display rounding:** per column decimals, default 0, floating up to 6 (OST-01-042).

## 5. Milestones

Each milestone ends with a commit, a demo JC can run, and green tests. The riskiest parts
(PDF speed, drawing feel) come first.

**M0. Scaffold (0.5 day)**
- Workspace and crates, app window opens, `cargo test` and `cargo clippy` pass.
- `run.bat` launcher that builds and starts the app.

**M1. Plan viewer (2 to 3 days)**
- Open a PDF, list pages, render the current page with zoom (fit, in, out, 100%, wheel)
  and pan (drag, wheel, Shift+wheel). Page Up and Page Down. Tile cache so a 36 x 48 sheet
  pans smoothly.
- Check: a real plan set opens, and a page flip takes under 1 second on JC's PC.

**M2. Project, pages, scale (2 days)**
- New, open, save project. Page list with editable sheet no. and name.
- Scale dialog: presets by style, apply to all pages, calculate from a known length,
  dimension tool to verify.
- Tests: parser, scale math, save and reload round trip.

**M3. Conditions (2 days)**
- Conditions panel: create, edit, duplicate, delete (the confirm names how much takeoff it
  removes). Properties: style, name, number, type, color, height, thickness, width, depth,
  slope, count shape, three result picks.
- Tests: result availability per style and shape (OST-05-036, OST-05-037).

**M4. Drawing takeoff (4 to 5 days)**
- Linear: drag for one segment, continuous mode (click points, double-click to end).
- Area: rectangle drag, polygon with auto close, self-crossing rejected, Esc steps back,
  backouts inside a selected area.
- Count: click to place.
- Snap angle 15 degrees, Shift disables it; Space toggles draw and select.
- Live length readout in the status bar.
- Tests: results of drawn shapes against hand-computed values.

**M5. Selecting and editing (3 days)**
- Click select, Shift+click toggle, rope select (touching mode). Move, nudge with arrows,
  vertex drag, add and remove vertex, delete. Reassign to another Condition of the same
  style. Mark negative.
- Undo and redo per page, cleared on page change (OST-09-005).

**M6. Bid areas and Summary (2 to 3 days)**
- Bid area list and active area picker; new takeoff takes the active area; assign selected
  takeoff to the active area; non-active areas drawn greyed.
- Summary grid: rows per Condition with up to three result columns, group by page, type,
  area, per column decimals.
- Tests: `insta` snapshots of the Summary for a fixture project.

**M7. Excel export and reference job check (2 days)**
- Export the Summary grid (current grouping) to `.xlsx`.
- Take off the reference job; a small comparison script reports every line that differs
  from the OST export by more than rounding. Fix and repeat until clean.

**M8. Ship for JC (1 day)**
- Release build, `REA Takeoff.bat` launcher and Desktop shortcut, one-page guide
  `docs/guide/takeoff-quickstart.md`, check on the smallest screen.

Pace: roughly one working session per milestone (M0 takes minutes). The real limits are
JC's hands-on feedback on drawing feel (M4, M5) and the reference exports (M7).

## 6. Verification approach

- **Unit tests** in `core` for every formula, using the spec's worked examples.
- **Golden tests** in `tests/`: a fixture project with known shapes; its Summary is
  snapshot-tested.
- **Black-box check** (M7): the same plan PDF taken off in OST and in our app, exports
  compared. Each mismatch goes back to the spec Unknowns register with the observed answer.
- **Run the app** before calling any milestone done: build, launch, use the feature,
  screenshot.
- `cargo clippy -- -D warnings` and `cargo fmt --check` on every commit.

## 7. Ledger items covered by Phase 1

- Units and entry: OST-01-034, OST-01-038 to OST-01-042.
- Projects and pages: OST-03-002, OST-03-007, OST-03-010, OST-03-016, OST-04-016 to
  OST-04-019.
- Viewing: OST-01-019, OST-01-021, OST-04-021, OST-04-022.
- Scale: OST-06-003, OST-06-012, OST-06-015 to OST-06-019.
- Conditions: OST-05-012, OST-05-014, OST-05-015, OST-05-019, OST-05-021 to OST-05-023,
  OST-05-031 to OST-05-037.
- Drawing: OST-06-072 to OST-06-074, OST-06-089 to OST-06-094, OST-06-098, OST-06-101,
  OST-06-109, OST-06-110.
- Editing: OST-09-001, OST-09-002, OST-09-005, OST-09-008 to OST-09-013, OST-09-039 to
  OST-09-042.
- Bid areas: OST-07-001, OST-07-003, OST-07-011 to OST-07-014.
- Summary and export: OST-12-002, OST-12-008, OST-12-009, OST-16-011.

## 8. Risks and how we handle them

- **PDF render speed on large sheets.** Proven first in M1 with tiles and a cache.
  Fallback: low-resolution preview while zooming, full resolution when idle.
- **pdfium.dll distribution.** Ship the DLL next to the exe; the launcher checks for it.
- **egui grid limits for the Summary.** The Phase 1 grid is simple. If Phase 2 needs
  spreadsheet behavior, evaluate a richer table crate or a web view for that panel only.
- **Formula gaps (Unknowns register).** Phase 1 avoids the unknown areas (attachments,
  zones, typicals, per-segment rounding). U-01 (entry parsing edge cases) is closed by
  typing test values in OST.
- **No reference job yet.** M0 to M6 run on any plan PDF. M7 needs JC's exports.

## 9. Decisions JC can change

- UI toolkit: egui recommended. Say so if you prefer a web-style UI (Tauri).
- Project storage: one file per project recommended (easy to copy and back up). A shared
  database for several estimators comes in Phase 4.
- Code location: `app/` folder in this same repo, recommended.
