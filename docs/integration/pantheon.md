# Integrating OpenTakeoff into Pantheon (Rust)

This guide shows how Pantheon can reuse what OpenTakeoff already knows about On-Screen Takeoff
(OST) and QuickBid: reading their job files, and computing the same quantities OST stores.
Everything lives in this repo (`deusmac/REA-OnscreenTakeoff`); nothing has to be worked out
again.

## 1. What you get

- **`rea-core`**: the data model (Project, Page, Scale, Condition, Takeoff, BidArea) and all
  quantity math. `rea_core::ost_totals` reproduces 99.82% of the 46,173 quantities stored in
  JC's 16 OST databases (rules and evidence in `docs/study/formats/ost-quantities.md`).
- **`rea-ost`**: reads OST `.osp` packages and bare `.ost` job files into the model, plus the
  raw OST fields the totals need.
- **`rea-quickbid`**: reads QuickBid `.qbp` bid packages (conditions, items, sizes, details,
  markups, wages, equipment, others). Personal data (contacts, phones, emails) is skipped.
- **`rea-pdf`**: plan PDF rendering on Pdfium (optional, only if Pantheon has no renderer).
- **`rea-store`**: the `.reaproj` SQLite project file (optional).
- **`rea-export`**: Summary to `.xlsx` (optional).

Knowledge docs, for reading rather than linking:

- `docs/spec/ost-features.md` and `docs/spec/quickbid-features.md`: every feature and behavior
  from both public user guides, with ledger IDs.
- `docs/study/formats/ost-package.md`, `ost-quantities.md`, `quickbid-package.md`: the file
  formats and quantity rules, proven on real jobs.

## 2. Add the crates

The crates sit in the `app/` workspace of a private repo. Cargo can pull them by git:

```toml
# Pantheon Cargo.toml
[dependencies]
rea-core = { git = "https://github.com/deusmac/REA-OnscreenTakeoff", rev = "<commit>" }
rea-ost = { git = "https://github.com/deusmac/REA-OnscreenTakeoff", rev = "<commit>" }
rea-quickbid = { git = "https://github.com/deusmac/REA-OnscreenTakeoff", rev = "<commit>" }
```

- Pin `rev` to a commit (not the branch) so a later change here cannot shift Pantheon's
  numbers without you choosing to update.
- The repo is private, so let Cargo use your git login. In Pantheon's `.cargo/config.toml`:

  ```toml
  [net]
  git-fetch-with-cli = true
  ```

- Alternative while both repos sit on the same PC: path dependencies, for example
  `rea-core = { path = "../REA-OnscreenTakeoff/app/crates/core" }`.

## 3. Open an OST job and get OST's numbers

```rust
use rea_core::ost_totals::{bid_totals, condition_totals, display_unit};

let imp = rea_ost::import_path(std::path::Path::new(r"E:\jobs\job.osp"))?;
let bid = &imp.bid; // imp.alternates holds alternate bids, same shape

// Model data for Pantheon's own UI.
for c in &bid.conditions { /* c.id, c.number, c.name, c.style, c.color, c.height, ... */ }
for t in &bid.takeoffs { /* t.page_id, t.condition_id, t.negative, t.geometry */ }

// OST's totals, computed with OST's rules.
let job = rea_ost::totals_job(bid);
let rows = bid_totals(&job); // per page, zone, area, typical area, condition
let per_condition = condition_totals(&job, &rows); // condition id -> [Option<f64>; 3]

for oc in &job.conditions {
    if let Some(q) = per_condition.get(&oc.uid) {
        for i in 0..3 {
            if let Some(v) = q[i] {
                let (unit, divisor) = display_unit(oc.results[i], oc.uoms[i]);
                println!("condition {} slot {}: {:.2} {unit}", oc.uid, i + 1, v / divisor);
            }
        }
    }
}
```

Plan PDFs come inside the package as `imp.plan_files` (archive path plus bytes); nothing is
written to disk by the library. Match a page to its file with
`rea_ost::plan_archive_path(&page.file_path)`, and use `page.file_page_index` (0 based) for the
page inside that PDF.

## 4. Units and coordinates (the part most likely to bite)

- **Model geometry** (`Takeoff.geometry`) is in sheet points: 1/72 inch on the printed sheet,
  origin top left, y down.
- **Page scale**: `page.scale.real_per_page_unit` is real inches per sheet point. Real length
  = point length times that value.
- **OST positions** (`OstTakeoff.position`, used by `ost_totals`) are already real inches. OST
  stores them that way; the importer converts them to sheet points for the model only.
- **Totals** come out in OST base units: inches, square inches, cubic inches or counts.
  `display_unit(code, uom)` returns the unit label and the divisor (for example LF and 12).
- **QuickBid lengths** look like tenths of a millimeter (a height of 33528 is 11 ft). Treat
  that as probable, not proven.

## 5. Read a QuickBid bid

```rust
let bytes = std::fs::read(r"E:\jobs\job.qbp")?;
let pkg = rea_quickbid::read_package(&bytes)?; // also accepts the raw XML
let s = rea_quickbid::summarize(&pkg);
// s.bid_no, s.bid_total, s.conditions (per condition stored mat_total, lab_total), ...
```

What is known and what is not:

- Conditions link to items through `BidDetails` (rows with a condition UID, item, size, wage
  type, quantity method). Stored `MatTotal` and `LabTotal` per condition are read as stored.
- The bid total adds markups, equipment, others and the markup clean/stock tables on top of
  the condition totals. That formula is not worked out yet, so Pantheon should show
  QuickBid's stored totals and not recompute them.

## 6. Keep Pantheon's numbers honest

`app/crates/core/tests/ost_parity.rs` is the proof that the rules match OST. To keep that
proof after integration:

1. Export an answer key from any OST database copy: `tools/ost-answer-key.ps1 -Mdb <copy.mdb>
   -Out <folder outside the repo>` (needs the 64-bit Access ACE OLEDB provider).
2. Run `cargo test --release -p rea-core --test ost_parity -- --nocapture` with
   `OST_ANSWER_KEY` set to that folder. It prints the match rate per condition type.
3. Re-run it whenever Pantheon moves to a newer `rev`.

## 7. Rules Pantheon must follow

- Work on copies of OST and QuickBid files, never on the live `C:\OCS Documents` folder (OST
  may have them open).
- Never commit job files or answer-key CSVs to any repo: they hold client data.
- `.ost` packages can carry an employee record with a password field. The importer skips it;
  do not add code that reads it.
- Plan paths inside job and project files can come from other people. Open only local drive
  paths (see `local_plan_path` in `app/desktop/src/project.rs`): a `\\server` path would make
  Windows send the user's login to that server.
- If Pantheon uses `rea-pdf`: ship `app/vendor/pdfium/pdfium.dll` next to the exe, and call
  Pdfium from one thread only (two threads at once crash it).

## 8. Known gaps (as of 2026-10-22)

- Not modeled: attachments (area codes 12 and 33), Round Quantity on linear conditions,
  typical page repeats, TIFF plan pages.
- About 0.18% of OST's stored quantities still differ, mostly rows OST itself never
  recalculated (list in `ost-quantities.md`).
- `.osp` packages carry no stored totals and their zone and typical-area sections were empty
  in all 37 of JC's packages, so those rules are proven on databases, not packages.
- The QuickBid bid-total formula is open.
