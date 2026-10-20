# CLAUDE.md - REA OnScreen Takeoff (clean-room takeoff + bid app)

> Persistent project memory. Read this file FULLY before doing anything else.
> It reflects the complete current state of the project, not the original plan.
> Companion doctrine: `THINKING.md` in this repo root (how to think, debug, and
> communicate). If you cannot access that file, the condensed rules in section 2
> below are binding on their own.

---

## 0. COLD-START BRIEFING (keep this section current, newest truth wins)

**Every agent updates this section before ending a session.** A new agent must be able
to read only section 0 and know exactly where things stand.

- **Project:** OpenTakeoff (JC's working name, 2026-10-20). A clean-room rebuild of the
  On-Screen Takeoff (OST) feature set, later joined by a QuickBid-style bidding module, in
  one application for JC's estimating work. Goal: one-to-one parity, and full compatibility
  with OST and QuickBid project files (open their jobs, same numbers).
- **Current phase:** Phase 1. M0 and M1 (plan viewer) done; core math, data model, SQLite
  store and Excel export landed as libraries (55 tests). Specs: `docs/spec/ost-features.md`
  and `docs/spec/quickbid-features.md`, evidence ledger in `docs/study/`.
- **Stack:** Rust (JC's decision, 2026-10-16). Rust core for geometry, scale and quantity
  math; PDF renderer and desktop UI to be chosen in the Phase 1 plan.
- **Method (clean-room evidence loop):** ledger IDs per behavior, unknowns register,
  black-box comparison of our exports against OST and QuickBid exports for the same job,
  coverage score per phase. Inputs are public docs, videos and JC's exports only.
- **Where the work lives:** GitHub `deusmac/REA-OnscreenTakeoff`, branch
  `claude/rea-tools-software-analysis-rk9nak`. No PR yet. Working copy on JC's Windows PC
  is cloned from that branch.
- **How to run it:** double-click `run.bat` in the repo root (builds release, then opens
  the OpenTakeoff window; a PDF path as argument opens it). Code lives in `app/` (Cargo workspace: `crates/core`, `store`,
  `pdf`, `export`, and `desktop`).
- **How to verify changes:** in `app/`: `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  then run `run.bat` and screenshot the window (`docs/screens/`).
- **Last session did:** (PC session, 2026-10-16) Downloaded every public article of both
  user guides (from the site sitemap, to the session scratchpad only, never committed),
  extracted paraphrased ledgers with 10 parallel readers, rewrote the OST spec and wrote
  the QuickBid spec. Ran an overlap check (no copied prose in committed docs).
- **Next up (priority order):**
  1. JC: export one real past job from OST (Summary and Estimate grids to Excel) and the
     same job from QuickBid (bid summary, job cost summary, job data export), plus its plan
     PDFs, into `reference/`. Closes the Unknowns registers and is the verification
     baseline.
  2. OST and QuickBid file compatibility: JC's job files are in `C:\OCS Documents` (OST
     `.mdb` and `.osp`, QuickBid `.qbp`). Map the formats from copies, write the format
     notes in `docs/study/formats/`, then build importers.
  3. M2 (project, pages, scale UI) wired to the new store and core modules. See
     `docs/plan/phase-1-rust.md`.
  4. Optional: training videos to close unknowns the guides do not cover.
- **Landmines:**
  - Read section 5 clean-room rules before touching OST or QuickBid in any way.
  - JC's PC has a machine-wide `CARGO_TARGET_DIR` pointing at another project
    (`E:\cargo-targets\meetily`). C: is nearly full (filled to 0 bytes on 2026-10-20), so
    builds go to `E:\cargo-targets\opentakeoff`: `run.bat` does this, and in a shell set
    `$env:CARGO_TARGET_DIR="E:\cargo-targets\opentakeoff"` first. Subagents in worktrees
    must share that folder, never build into their own worktree on C:.
  - Pdfium crashes when two threads use it at once (even with `thread_safe`). Keep all
    `rea-pdf` calls on one thread; its tests serialize on a mutex.
  - egui/eframe 0.35 API: implement `App::ui(&mut self, ui, frame)` (not `update`), and
    use `egui::Panel::top/bottom/left` with `.show(ui, ..)`.
  - The Bash tool rewrites `>nul` to `/dev/null` and backslash escapes inside written
    text. Write `.bat` files with PowerShell or the Write tool, never via Bash.
  - Raw guide article text is copyrighted. Keep it in the session scratchpad; commit only
    our paraphrased ledgers and specs.
  - JC asked that OST study NOT drive his PC (no Windows-MCP, computer-use, desktop
    screenshots, or launching OST). Use public docs and YouTube only.

## 1. Who you are working for

John Carlo Tiempo (JC), engr.jctiempo@gmail.com. Estimator and pre-construction manager at
Brightline Contracting, a commercial drywall, metal framing, and painting subcontractor in
Greenville, South Carolina. He works remotely from the Philippines (Asia/Manila). Strong
Python developer, pragmatic, ships tools for real construction workflows. His boss Jack
Morgan often reads the outputs on a phone. Full people-and-vendor context lives in
`C:\Users\User\Desktop\CLAUDE CONTEXT\CLAUDE.md`; read it when the task touches emails,
invoices, GCs, or company data.

**Style: act first, explain after.** JC grants initiative. Fix bugs and build improvements
without asking, within section 5 guardrails. No planning theater. Tight summaries.

## 2. The rules (condensed doctrine, binding for any model)

1. Understand before acting. Read this file fully. Read files before editing them.
2. Root cause before fix. Reproduce, isolate, prove the hypothesis, THEN fix. Never stack
   failed fixes; revert each failed attempt. Three strikes on a bug means write down what
   you learned here and regroup.
3. Plan before code for anything beyond a one-file change. Write the plan down.
4. Evidence before claims. Never say done or fixed without having run it and seen it work.
   Verify against real data when it exists.
5. Simplest thing that works. No speculative abstraction, dependencies, or config.
6. Git from minute one. Commit every working increment. Branch per feature.
7. Security ships with the feature. Every new route, query, or mutation gets its auth check
   and input validation the same day it is written. An endpoint without auth is a bug.
8. Instant UI feedback. Every user-facing write reacts immediately (optimistic update or
   loading state). Never a silent wait on a network round trip.
9. Cost discipline. Do not re-read specs already summarized here. Batch questions and
   parallel-run independent commands. No new MCP servers, plugins, or dependencies unless
   the task is impossible without them. Use cheap models or subagents for research and
   bulk work when the harness allows choosing.
10. Output conventions, everywhere, including code comments and product copy: no em dashes,
    no emojis, lists not tables in prose (tables inside app UIs are fine), phone numbers
    without a leading plus sign, plain professional tone in anything boss- or customer-facing.

## 3. Session protocol (this is how sessions get saved, do not skip)

**On start:**
1. Read this entire file.
2. If resume, memory, or checkpoint skills are available, use them. Otherwise section 0
   and section 9 ARE the memory.
3. Pull the branch first (`git pull`), since cloud and PC sessions both push to it.
4. State in one line what you are about to do, then do it.

**On end (mandatory, the session is not over until this is done):**
1. Update section 0 (cold-start briefing) to the new truth.
2. Add an entry to section 9 (changelog): date, what shipped, what broke, what is next.
   Section 9 is this project's changelog; there is no separate CHANGELOG.md.
3. Commit everything, including this file, and push the branch.
4. If checkpoint or memory-save skills are available, run them too.

## 4. Project specifics

- **Goal:** One app covering the full OST feature set first, then QuickBid-style bidding on
  the same data model, so quantities flow straight into bids with no second app.
- **Planned data model:** Project, then Plan sets and pages (with scale), then Conditions
  (linear, area, count, with height, waste and other multipliers), then Takeoff shapes,
  then Quantities, then Bid items, then Pricing (labor, material, markup).
- **Phases:**
  1. Phase 0: study OST and QuickBid behavior, write the spec.
  2. Phase 1: takeoff MVP (open PDF plans, set scale, linear/area/count conditions,
     totals, Excel export).
  3. Phase 2: rest of the OST feature set from the spec.
  4. Phase 3: bidding module (QuickBid equivalent) on the same data.
  5. Phase 4: hook into the company systems (for example the Athena bid estimate).
- **Stack:** Rust (decided by JC 2026-10-16). UI and PDF renderer chosen in the Phase 1
  plan.
- **Data sources:** Reference job files in `reference/` (once added). OST and QuickBid on
  JC's PC are study subjects only, never data sources for code.
- **Key files:** `CLAUDE.md`, `THINKING.md`, `docs/spec/ost-features.md`,
  `docs/spec/quickbid-features.md`, `docs/study/ost/*.md` and `docs/study/qb/*.md`
  (evidence ledgers; spec bullets cite their IDs such as `OST-05-015`, `QB-08-015`).
- **External services and keys:** None yet. Never hardcode keys, never commit .env.
- **Conventions in this codebase:** To be set with the stack.
- **Skills for this project** (global skill routing in `~/.claude/CLAUDE.md` applies too):
  `superpowers:writing-plans` for each milestone plan, `superpowers:test-driven-development`
  for core math (tests cite ledger IDs), `ecc:rust-build` on build errors, `ecc:rust-review`
  before each milestone commit, `verify-done` plus a `run.bat` screenshot before claiming done.
  The changelog is section 9 of this file; do not create `CHANGELOG.md`.

## 5. Guardrails (do not cross without JC's explicit go-ahead)

**Clean-room rules (this project lives or dies on these):**
- Never decompile, disassemble, debug-attach to, or unpack OST or QuickBid binaries, DLLs
  or installers.
- Project files: JC gave the go-ahead (2026-10-20) to read his own OST and QuickBid job files
  for import/export compatibility. Work on copies only, never inside `C:\OCS Documents`.
  Commit format notes in our own words; never commit raw job files (client data).
- Study only through observable behavior and published material: using the app, screenshots,
  the user manuals and help files, training videos, and the exports the apps produce
  (Excel, CSV, PDF reports).
- Never copy their code, icons, images, UI artwork, help text, product names or branding
  into this repo. Specs describe behavior in our own words.
- Raw text of vendor help articles stays out of the repo. Commit paraphrased ledgers only.
- Never modify the OST or QuickBid installation or JC's real project files. Work on copies.
- Before committing reference files, check they hold no client-confidential data JC does
  not want in GitHub. Keep the repo private.

**General:**
- Never write to live company data (production sheets, real email sending, real Drive files)
  during testing. Stage or dry-run instead.
- Never delete or overwrite originals of anything (emails, PDFs, backups).
- Never commit secrets. Never export credentials embedded in workflow or config files.

## 6. Verification playbook

No build tools yet. Minimum bar until they exist: run the program on a realistic input (a
real plan PDF from `reference/`) and paste the output into the session summary. Quantities
must match the OST export for the same job within rounding.

## 7. Shipping for colleagues

Anything a non-developer will use gets, before it is called finished:
- A one-click launcher (bat, vbs, or shortcut) that just works.
- A short plain-English guide (one page, no jargon) next to it.
- A test on the smallest screen it will realistically be used on.

## 8. Backlog

1. Reference job in `reference/` (OST and QuickBid exports of the same job plus plans).
2. Close the Unknowns registers in both specs against the reference job.
3. Phase 1 build per `docs/plan/phase-1-rust.md` (plan drafted, awaiting JC review).
4. Training videos as a second source for unknowns the guides leave open.

---

## 9. SESSION CHANGELOG (newest first; never delete old entries)

### 2026-10-20 - Session 4 (PC)
- Shipped: skill routing rules in global `~/.claude/CLAUDE.md` and a project skills line in
  section 4. JC chose not to apply the cloud setup script; skills are used per task instead.
- Shipped: M1 plan viewer (Pdfium 7881 bundled; whole-page image plus sharp render of the
  visible area; Ctrl+wheel zoom, wheel and Shift+wheel pan, right/middle drag, Page Up/Down,
  * + - / keys). Verified on a real 33-page 36 x 24 permit set: page flip 269 to 903 ms,
  sharp at 239% (`docs/screens/m1-viewer-*.png`). Five parallel agents added core scale,
  linear, area, count, summary, data model, SQLite store and Excel export; merged, 55 tests.
- Decided: name is OpenTakeoff. JC allowed reading his own OST/QuickBid job files for
  compatibility (copies only). REA tool lines removed from CLAUDE.md at JC's request.
- Broke: C: drive filled to 0 bytes from per-worktree builds; cleaned, builds moved to E:.
- Shipped later the same session: `rea-ost` (.osp/.ost importer), `rea-quickbid` (.qbp
  reader, contacts and personal fields skipped), OST answer-key harness
  (`tools/ost-answer-key.ps1`, `crates/core/tests/ost_parity.rs`, data in
  E:\opentakeoff-study\answer-key, run with OST_ANSWER_KEY set). Our core reproduces 1962 of
  1971 OST stored totals from the vendor sample DB; only area grid length (code 16) misses.
  Format notes in `docs/study/formats/`. OST positions are real inches, not sheet units.
- Corpus run (same day): all of `C:\OCS Documents` copied to E:\opentakeoff-study\corpus.
  OST reader opens 37 of 38 .osp (The Loom is part 2 of a split CAB, part 1 missing; clear
  error now). QuickBid reader opens 17 of 17 .qbp. Parity over 16 OST databases (46,177
  stored quantities): 97.3% match. Count 99.6%, type-4 count 100%, attached 98.4%,
  linear 97.6%, area 90.5%. Scan tools: `cargo run --release -p rea-ost --example scan --
  <folder>` and the same for `rea-quickbid`. Per-database results: answer-keys\*\parity.txt.
- Parity gaps closed by four parallel agents (rules in `docs/study/formats/ost-quantities.md`):
  Round Quantity (per-edge round-up walk) and grid origin at the area centroid (codes 16,
  18); zones (a zone row holds the quantity, its no-zone sibling stores 0) and stale rows;
  linear arc height is the last position number, typical-group takeoffs are not in page
  totals; count shape 12 perimeter and codes 25, 26. Parity over all 16 databases:
  46,089 of 46,173 (99.82%). Count 100%, type 4 100%, linear 99.79%, area 99.55%, attached
  99.53%. Residuals are mostly rows OST never recalculated; list in the notes.
- M2 shipped: File > Open (or a path argument to run.bat) takes an OST .osp. Plan PDFs are
  unpacked to %TEMP%\opentakeoff\<job>, pages follow OST's order, takeoffs draw in their
  condition color over the plan, and a right-hand Summary lists each condition (LF and wall
  SF, SF and perimeter, EA) from our core math (`desktop/src/quantities.rs`, one test).
  Verified on the real Sweat 440 job: 33 pages, 36 conditions, takeoffs sit exactly on the
  walls of A-1.0 (`docs/screens/m2-osp-*.png`).
- OST total rules moved into `crates/core/src/ost_totals.rs` (`bid_totals`,
  `condition_totals`, `display_unit`); the parity harness and the desktop Summary both call
  it, parity unchanged at 46,089 of 46,173. Summary shows Qty 1 to 3 with OST's units
  (`docs/screens/m2-summary-full.png`). The .osp XML carries no stored totals and its zone
  and typical-area sections are empty in all 37 packages, so those rules are untested on
  packages (they work on the databases).
- M3/M4 shipped (desktop split into doc, geom, project, tools, viewer, conditions_ui):
  New project from PDF, Open/Save/Save As .reaproj (plans from an .osp go to
  <project>_plans next to it), unsaved-changes prompt, page scale presets and Calibrate,
  conditions panel (new, edit, duplicate, delete), draw linear/area/count with the left
  mouse, Select (V) with hit test, Delete, vertex drag, Ctrl+Z/Ctrl+Y, live Summary.
  Verified in the release build on Sweat 440 (`docs/screens/m3-*.png`), 15 desktop tests.
  Security: plan paths from project/job files must be local absolute paths (no \\server,
  no ..), `project::local_plan_path`.
- Broke: during the real-app test some synthetic clicks landed in JC's QuickBid window;
  JC told to check that job. Drive the app only when it is verified to be in front.
- Next: .reaproj does not yet store arcs, grid sizes, zones, typical areas, parent links
  and unnamed result codes, so an .osp saved as .reaproj drifts on those rows; backouts,
  snap, rectangle drag, multi-select, copy/move not built; TIFF pages blank; QuickBid
  bid-total formula.

### 2026-10-16 - Session 3 (PC)
- Shipped: `docs/spec/ost-features.md` second draft and `docs/spec/quickbid-features.md`
  first draft, both from a full read of the public guides (555 articles). Evidence ledgers
  in `docs/study/`. Clean-room overlap check run on committed docs (no copied prose).
- Decided: stack is Rust. Method is the evidence loop (ledger, unknowns, black-box
  comparison, coverage).
- Broke or blocked: firecrawl rate limits gave summaries only, so direct fetch from the
  site sitemap was used instead. One reader pass hit the usage limit and was resumed. JC
  asked to use the REA MCP on OST and QuickBid binaries; declined (clean-room and
  copyright), recorded as a landmine.
- Also shipped: `docs/plan/phase-1-rust.md` (Phase 1 Rust plan: workspace layout, crates,
  data model, core math with ledger IDs, milestones M0 to M8, verification, risks).
- Also shipped: M0 scaffold. Cargo workspace in `app/` (eframe 0.35), core feet-inch
  entry parser with 6 tests from ledger examples, `run.bat` launcher; window verified
  running (screenshot `docs/screens/m0-window.png`). Tests, clippy and fmt clean.
- JC confirmed the goal: functional one-to-one parity with OST and QuickBid (features,
  workflow, matching numbers), with our own code and visuals. Recorded in the plan.
- Next: M1 plan viewer; JC adds reference exports.

### 2026-10-15 - Session 2 (PC)
- Shipped: `docs/spec/ost-features.md` first draft (overview, storage, plans, scale,
  conditions, takeoff tools, automation, typicals, bid areas and zones, annotations,
  Summary, Estimate, masters, reports, open questions, Phase 1 MVP mapping).
- Broke or blocked: decompiling OST binaries was blocked by the safety classifier and
  abandoned. The UI walk on JC's PC was partial (main window, tabs, toolbars, left panel
  only); JC then asked that study use docs and videos only, not his PC.
- Next: JC adds a reference job export to `reference/`; then stack decision and Phase 1 plan.

### 2026-10-15 - Session 1 (cloud)
- Shipped: repo setup with CLAUDE.md (from JC's universal template), THINKING.md, .gitignore.
- Learned: OST is Windows only, so the study phase must run on JC's PC. Clean-room rules
  agreed: behavior and docs only, no decompiling.
- Next: open the repo in Claude Code on the PC and start the OST behavior study.
