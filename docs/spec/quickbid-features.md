# QuickBid feature spec (Phase 0, clean-room)

Status: first draft, 2026-10-16. Built from a full read of the public QuickBid user guide
(all 15 categories, 268 articles).

This document describes what the reference bidding product does, in our own words, so we
can build an equivalent bidding module on the same data model as our takeoff module. It is
a behavior spec, not a design. Companion spec: `ost-features.md` (takeoff side).

## Sources and method

- Public user guide on the vendor help site, every article in categories 01 to 15, read in
  full and paraphrased into the evidence ledger `docs/study/qb/`. Every bullet there has a
  ledger ID such as `QB-08-015`; this spec cites them.
- Not used, by rule (CLAUDE.md section 5): binaries, installers, databases, decompilers,
  disassemblers, debuggers or any reverse-engineering tool pointed at the product. No help
  text, icons or artwork copied. Raw article text is not committed.
- Same evidence loop as the takeoff spec: ledger, unknowns register, black-box comparison
  against JC's real bid exports, coverage score.

Confidence tags: [docs] stated in the guide; [inferred] our reading, needs a real export to
confirm; [unknown] listed in the Unknowns register.

## 1. What the bidding side does

Takes Condition quantities (from takeoff, a quantity survey, or typed) and prices them with
Items (material and labor), adds equipment, subcontractor and other costs, applies indirect
expenses and markups per CSI section, and produces a selling price, reports, proposals, a
schedule of values and accounting exports. [docs] QB-01-010, QB-06-001

## 2. Object model

- Database: bids, masters, options. Imperial or metric fixed per database. [docs]
  QB-01-035, QB-15-001
- Bid = Base Bid plus child bids. Alternates share payroll settings and material pricing
  with the base, both ways; markups and indirects are their own. Change Orders have their
  own payroll settings, material pricing, markups, indirects and totals, seeded from the base
  bid first and then from masters. [docs] QB-06-040 to QB-06-043
- Section = CSI MasterFormat division and section (5, 6 or 8 digits, first two = division).
  Every cost belongs to a section. Markups and indirects are set per section. [docs]
  QB-04-023, QB-02-034
- Condition = one priced building object: number, name, type, area allocation, Qty 1 to 3
  each with a UOM, one height, width and depth for EA conditions, notes, verified flag, and
  a grid of Item lines. One Condition per object type and per height. [docs] QB-07-001,
  QB-07-028
- Item = material, labor or both, with one UOM. Three levels: Master item (template),
  bid-level copy made on first use (isolated from later master edits), and the line inside a
  Condition. [docs] QB-05-001, QB-05-011 to QB-05-016
  - One material price and one waste factor per item (and size) per bid, shared by the base
    and its alternates. Change orders keep their own. [docs] QB-05-016, QB-02-048,
    QB-09-014
  - Per line, independent of other uses: section, labor cost code, height, OC, layers,
    production, size or style. [docs] QB-05-015, QB-07-051
- Subs, Equipment and Other rows live on their own tabs, each with a section and an area
  allocation. [docs] QB-02-023, QB-02-029, QB-02-032
- Masters: items, contacts, contact types, manufacturers, item types, containers, divisions
  and sections, cost codes, account codes, payroll classes, estimators, job statuses, job
  types, condition types, assemblies. [docs] QB-04-001
- Database settings (not masters): payroll settings (wage types, burdens), default markups
  and indirect expenses, new bid options. [docs] QB-04-003, QB-03-001

## 3. Units, entry, rounding

- Units: EA, IN, LF, LY, IN2, SF, SY, SQ (100 SF), CF, CY; metric EA, mm, m, mm2, m2, mm3,
  m3. Packed feet-inches entry (last two digits inches) and millimeters, same as takeoff.
  [docs] QB-01-036 to QB-01-039
- Full precision stored; display decimals per column (0..n or floating up to 6),
  program-wide. Markup entry boxes always show two decimals. Reports and exports use fixed
  two decimals or whole numbers. [docs] QB-01-001 to QB-01-009
- Internal rounding to cents at line level before summing. [unknown] QB-U-01

## 4. Item quantity math (Calculation tab)

Inputs: the Condition quantity chosen per item (Qty 1, 2 or 3; default Qty 1), height,
OC (on center), layers (rows), waste percent, installation method. [docs] QB-05-037,
QB-05-039

Methods:
- Standard formula: item_qty = cond_qty * height / OC * layers, then waste. [docs]
  QB-05-041
  - Installation method drops terms (removed, not set to zero): none checked = full
    formula (studs, wall covering); Horizontal = cond_qty / OC * layers (grid, channel);
    Continuous = cond_qty * height * layers (board, insulation, paint); both =
    cond_qty * layers (flooring). [docs] QB-05-056 to QB-05-061
- Quantity x Height: item_qty = cond_qty * height * layers, then waste. Only for item UOM
  EA, IN, LF, LY. [docs] QB-05-044
- Default to Condition Quantity: item_qty = cond_qty (meant for EA items on count
  conditions). [docs] QB-05-043
- Enter Direct Quantity: user types the quantity (starts as a red 0). The app keeps the
  ratio entered / cond_qty and rescales when the condition quantity changes. Typing 0
  returns to the calculated value. [docs] QB-05-042, QB-07-059
- On-center exception factors: when a line's OC is exactly 16 in or 24 in (400 or 600 mm),
  item_qty = cond_qty * height * factor * layers, and both the OC term and waste are
  ignored. [docs] QB-05-063 to QB-05-066
- Waste applies to the Standard and Quantity x Height methods only. Assumed item_qty *
  (1 + waste/100). [inferred] QB-05-046, QB-U-02
- Labor on waste: bid option. On = labor uses the quantity with waste; off = labor uses the
  quantity without waste while the grid shows the quantity with waste. [docs] QB-05-047,
  QB-03-006
- Item default entries (section, labor cost code, height where 0 = use condition height,
  repeat every, OC, rows/layers) are applied when the item is added, then editable per line.
  An item default height overrides the condition height for that line (sheet goods sold in 8
  ft). [docs] QB-05-052 to QB-05-054
- Count conditions with width and depth: perimeter = 2 * (W + D); side area = H *
  perimeter; volume = H * W * D, so any item UOM can derive from a count. Can be switched
  off per bid. [docs] QB-07-031, QB-07-032
- Report Data Checker warns when an item calculates from a zero or missing condition
  quantity. [docs] QB-12-017

## 5. Sizing, containers, mixtures, chains

- Size methods: Sizes (length, or length x width for area items), Styles (grade, color,
  method; price and labor only, no quantity effect), Styles with Sizes, No Sizes, Mixture.
  [docs] QB-05-069 to QB-05-081
- Units per container converts item quantity to containers; the container name is a label
  only. [docs] QB-05-071, QB-05-076
- Round to whole containers (all nearest, all up, all down) works by changing the item's
  waste factor, never below 0. [docs] QB-09-011 to QB-09-013
- Mixture: a yield (for example 1,000 SF) and component items with quantities in each
  component's container units; can convert UOM (LF in, EA out). [docs] QB-05-083 to
  QB-05-085
  - containers = item_qty / yield * component_qty. [inferred] QB-05-088
- Chain: a lead item that inserts its chained items below it the first time it is added to
  a Condition, with auto size. [docs] QB-05-032 to QB-05-034
- Pieces per hour relate production to size: pieces = qty per hour / size area (125 SF per
  hour = 3.13 sheets of 40 SF). "View as cut size" uses the line height instead of stock
  length. [docs] QB-10-011, QB-10-012

## 6. Material pricing

- Price methods: per UOM (price per 1, 100 or 1000 units), per style, per piece (size), per
  container. Each price change stamps a date. [docs] QB-05-089 to QB-05-093
  - material_total = item_qty * price / per. [docs] QB-07-058
  - By container: containers = item_qty / units_per_container; cost = containers * price.
    Whether containers round up to whole units is not stated. [inferred] QB-05-093,
    QB-U-03
- Editing a price on a line asks whether to update the master. Yes = this bid (all uses)
  and the master; No = this bid only. Other bids never change. [docs] QB-07-055
- Refresh material prices resets the bid to master prices, unaccepts eQuotes and clears
  suppliers. Not undoable. [docs] QB-09-009, QB-09-010
- Update unit cost by percent on the visible items (undo only by the negative percent).
  [docs] QB-02-050
- Replace a material everywhere in the bid; requires matching setup, UOM, quantity source,
  size method and price methods; quantities, sections and labor stay. [docs] QB-09-002 to
  QB-09-008

## 7. Labor

- Payroll class: base wage, fees, insurance, tax per wage type; hourly and daily wages are
  computed. Bid-level copy on first use; "refresh payroll wages" pulls master values. [docs]
  QB-04-025, QB-04-026
- Wage types: named list (open, union, sub by default); the bid has a default wage type.
  [docs] QB-03-012, QB-06-012
- Burdens: up to eight lines, percent or amount, on base wage only (not compounded).
  amount_pct = amount / base_rate * 100; total_burden_pct = sum of all. One burden can be
  flagged as the override target. [docs] QB-03-013 to QB-03-016
- Crew: rows of payroll class and count; fractions that sum to 1 give a one-person
  composite crew with a blended rate. Hours split by fraction in reports. [docs] QB-05-098
  to QB-05-101
- Production: per hour, per day and unit price are linked (edit one, the others update).
  Price excludes burdens, markups and indirects. [docs] QB-05-103
  - hours = item_qty / production_per_hour; per_day = per_hour * hours_per_day; unit
    labor price = crew hourly cost / production per hour. [inferred] QB-05-104, QB-10-006
  - Layers multiply item quantity, so hours and labor total scale; production and unit cost
    do not. [docs] QB-10-005
- Adjusting production at three levels: per line (% Base, 100 = master base, red above,
  blue below), per Condition (Labor Adjust over one, selected or all labor codes, based on
  the Condition quantity, applied as the same percent to each line), per bid (Labor tab).
  [docs] QB-10-007 to QB-10-025
- Swap a labor cost code bid-wide; production and cost unchanged. [docs] QB-10-029
- Labor burden override: only in the all-sections view; puts one percent on the override
  burden and zeroes the others; alternates cannot override (shared with base). [docs]
  QB-10-032 to QB-10-037

## 8. Condition detail and line grid

- Line columns: section, item, description, labor code, height, OC, layers, qty, UOM,
  size, material price, per, labor cost, wage type, crew, % base, qty per hour, pieces per
  hour, crew hours, material total, labor total, item total. [docs] QB-07-053
- Line totals: mat_total = qty * price / per; lab_total = qty * labor unit cost (qty
  without waste when labor-on-waste is off); item_total = mat_total + lab_total (raw cost,
  no markups). Column subtotals at the bottom. [docs] QB-07-058
- Cell colors carry meaning (money, editable quantities, production above or below base,
  mixture, manual quantity, cannot-calculate). [docs] QB-07-052, QB-07-054
- Section totals panel: priced material and labor, selling price with markups, and price
  including a share of indirect expenses; unit price = section selling price / Qty 1 in the
  chosen view UOM. [docs] QB-07-047, QB-07-048
- Condition assemblies: saved item groups that store production rates and ratios (not
  costs); on insert, height-based items take the new condition height, prices come from the
  bid-level items first, then masters; append or replace. Equipment and Other assemblies
  are row groups. [docs] QB-04-032 to QB-04-037, QB-07-066 to QB-07-073
- Copy item rows between Conditions (also across bids and databases). [docs] QB-07-074
- Duplicate a Condition (keeps markups and indirects) versus copy to another bid (does not).
  [docs] QB-07-011, QB-07-014

## 9. Areas, typical areas, phases

- Bid areas: hierarchy, quick add, area size (SF) for analysis reports. A Condition can be
  allocated to one area or many (Qty 1 to 3 per area; the Condition quantity is the sum).
  [docs] QB-07-037, QB-07-041
- Typical areas: matrix of occurrences per bid area. Per-occurrence quantity is entered
  once; condition_qty = sum(per occurrence qty * total occurrences) + direct area rows.
  Shared items go to ordinary area rows. [docs] QB-07-042 to QB-07-044
- Phases roll areas up for accounting exports. [docs] QB-07-036, QB-07-046
- Filtering the Conditions tab by section or area reduces each total to that slice;
  detail opened from an area filter shows only that area and locks quantity. [docs]
  QB-02-013 to QB-02-018
- Subs, Equipment and Other rows allocate dollars to one area or many; default Unassigned.
  [docs] QB-11-002 to QB-11-004

## 10. Subs, equipment, other

- Subs: subcontractor, section, bid amount, net adjust percent, area or multi-area, MBE and
  WBE flags, cost code, notes, accepted flag. Only accepted subs count.
  adjusted = bid_amount * (1 + net_adjust/100). An optional bid worksheet fills the amount.
  Active Sections pre-list the sections that need sub coverage. [docs] QB-02-023 to
  QB-02-027, QB-03-026
- Equipment and Other rows: description, section, qty, unit, area, rate, cost code, labor
  rate, labor cost code, payroll class, wage type. total = qty * rate; labor = qty *
  labor rate. [docs] QB-02-029 to QB-02-032

## 11. Indirect expenses, markups, selling price

Order of operations per section: [docs] QB-08-003, QB-08-054, QB-08-004
1. Direct costs by category: material, labor, subs, equipment, other.
2. Indirect expenses Stock, Clean, Supervision (material and labor only).
3. Section markups: escalation, tax (material, subs, equipment, other) or burden (labor),
   overhead, profit.
4. Per diem (no escalation or tax/burden on it).
5. Additional markups on the bid total.

Indirect expenses: [docs] QB-08-019 to QB-08-053
- Each has an "M/U" flag: on = included in the subtotal that gets overhead and profit.
- Stock and Clean methods: material quantity (qty of a material cost code * fixed cost per
  unit), material cost (material dollars * percent), labor hours (hours * percent, costed
  at an installer payroll class). Result posts as material or labor cost.
- Supervision methods: labor cost (wages * percent), labor hours (hours * percent at a
  supervisor's wage), duration (selected installer labor days / crew size * supervisor base
  daily wage; daily wage = hourly base * hours per day). Duration is bid-only.
- Per diem: labor hours * per hour, per payroll class; per_hour = per_day / hours_per_day.
- Labor-based entries are set per payroll class and wage type combination.
- Defaults per section or for all sections apply when a section is first used. Editing in
  all-sections mode zeroes every section's indirects first, not undoable.

Section markups: [docs] QB-08-055 to QB-08-063
- Escalation: percent (can be negative) on all five categories; does not change item unit
  costs, does change selling unit cost.
- Tax/burden: material sales tax and tax on subs, equipment, other are editable; labor
  burden is the composite from payroll settings.
- Overhead is treated as cost; profit always applies on cost plus overhead.
- Straight: OH = cost * oh_pct; profit = (cost + OH) * profit_pct. Cost 2,000 at 20/20 =
  2,880. [docs] QB-08-015
- Gross: marked = base / (1 - pct). Cost 2,000 at 20/20: OH 500, profit 625, total 3,125.
  [docs] QB-08-016
- Method is chosen per bid (default from new-bid options); computed per category.
- Escalation and tax order relative to overhead. [unknown] QB-U-04

Additional markups: [docs] QB-08-064 to QB-08-070
- Rows of description, method, apply-to, value. Methods: amount, cost per thousand (value *
  base / 1000), percent (straight or gross per row), subtotal. Apply-to can be a system
  total or an earlier row, so rows can stack. Not touched by a selling price override.

Selling price: [docs] QB-11-005 to QB-11-013
- Typing a section or bid selling price back-solves one common profit percent across all
  categories; escalation, tax and overhead stay. Negative profit is rejected; 0 shows the
  lowest price. Change orders keep their own price; accepted alternates can be included.
- Project price = base + accepted alternates + change orders; base price alone differs.
- Exact back-solve and rounding of the resulting percent. [unknown] QB-U-05

## 12. eQuotes (supplier pricing requests)

- Group of materials sent to suppliers by email link; supplier fills a web form (price,
  notes, minimum order, good-through date); reply file imports into the bid; compare side by
  side; accept all or chosen items, which writes price and supplier to the Materials tab;
  overrides shown distinctly; last accepted wins. One eQuote per item per base plus
  alternates. Needs an internet service. [docs] QB-09-018 to QB-09-068
- For us: Phase 4 or later; a simpler "request quote by email, paste prices back" flow may
  cover JC's need first.

## 13. Tabs and views

- Bids (filters, group by status, eQuote counter), Conditions (filters, group by type,
  quick sum of shown or selected rows, verified flag), Subs, Equipment, Other, Markups
  (sections grid, markup detail, overview graph of the five categories, additional
  markups), Materials (one row per material and size; price, waste, rounding, supplier),
  Labor (one row per labor code, UOM and production; optional break by height), Payroll,
  Schedule of Values. [docs] QB-01-049, QB-02-003 to QB-02-061
- Bid navigator to switch bids and drag between databases. [docs] QB-02-009

## 14. Reports

[docs] QB-12-001 to QB-12-029
- Bid: cover page text, bid summary (conditions with unit selling prices, by area or
  section), section markup, job cost summary (after tax, before markup).
- Analysis: cost per square size (selling price / area size), labor risk ((overhead +
  profit) / burdened labor), cost percentages by category, unit price by cost code.
- Labor: field report, payroll report (base rate, burden, hours, gross), production report.
- Materials: vendor quote sheet, purchasing, stocking (no cost). No escalation in material
  reports.
- Run for the selected bid with section and area filters and options to include accepted
  alternates and change orders. Every tab prints and exports to PDF, Excel, Word.
- Custom report definitions with a parameter list (company, bid, filters). For us: Excel
  export first, report templates later.

## 15. Proposals and schedule of values

- Proposals: document templates with merge fields (cover sheet, job site, recipient,
  conditions list with decimals and unit price flags, equipment, other, totals, markups,
  alternates, change orders); one document per chosen recipient. [docs] QB-13-001 to
  QB-13-009
- Schedule of values: a billing contract built from the base bid plus accepted alternates
  and change orders, grouped by cost code type (labor, material, equipment, other, sub, tax,
  additional markup) or cost code description; editable overhead, profit and percent of
  contract per line; variance = contract total minus bid total, target zero. A bid belongs
  to one contract. [docs] QB-13-010 to QB-13-016

## 16. Imports and exports

- Bid package: whole bid with its payroll settings; import always creates a new bid; newer
  files cannot import into older versions; missing masters added; interactive takeoff link
  is cut. [docs] QB-14-010 to QB-14-021
- Bid from a takeoff file: options to import zero-quantity conditions, create unused areas,
  and round quantities to N decimals (max 4, default 0). Update from a takeoff file
  overwrites conditions and quantities and deletes conditions not in the file. [docs]
  QB-06-034 to QB-06-039
- Quantity survey import from Excel: header row 5 (type, area, name, item, description,
  quantity, unit); row markers T (type) and N (new condition); condition name = name +
  item suffix; only into an empty, non-linked bid. [docs] QB-07-015 to QB-07-025
- Accounting job budget exports for 14 accounting systems, plus material exports and a raw
  job data file. Key options: distribute tax and burden into cost amounts or emit separate
  rows (category codes MT, LB), use account codes instead of cost codes, group by area or
  phase. Category codes M, L, S, E, O. [docs] QB-14-030 to QB-14-266
- For us: one generic Excel job data export first (QB-14-263 field list is a good model),
  then the specific targets Brightline uses.

## 17. Takeoff link

- Takeoff side owns projects, child bids, Conditions, areas, typical areas and quantities;
  bidding side owns items, labor, materials, subs, equipment, other and markups. Linked
  Conditions cannot be added, deleted or have quantities edited on the bidding side.
  Interactive mode syncs live; manual mode on refresh. Zones arrive as top-level bid areas.
  Reconnect requires identical names. [docs] QB-06-019 to QB-06-033
- In our app both sides share one data model, so most of the link rules disappear. We keep
  the ownership rule (quantities come from takeoff) and the takeoff-file import for jobs
  done in other tools.

## 18. Masters, options, databases, security

- Masters are global except items, payroll classes and assemblies, which are copied into a
  bid on first use. Used records cannot be deleted; a global replace exists for some
  tables. Synchronize copies masters (upsert, contacts insert-only) between databases.
  [docs] QB-01-046, QB-04-004 to QB-04-008, QB-15-080 to QB-15-082
- New bid options: next bid number, bidding company, default wage type, hours per day,
  days per week, markup method, labor on waste, ignore width and depth, default indirects,
  cover page text. [docs] QB-03-005
- Databases: local single-user file or shared server; backups and compact; one editor per
  bid via bid locks that an admin can clear. [docs] QB-15-001 to QB-15-121
- Security: estimator access levels over five areas (master items, others' bids, bids list,
  master menu, payroll classes) with stated override order. [docs] QB-15-100 to QB-15-107

## Unknowns register

Closed by black-box comparison against JC's real bid exports (Excel and PDF reports for the
same job priced in the reference product).

- QB-U-01 Line-level rounding to cents before summing. (QB-01-001, QB-01-007)
- QB-U-02 Exact waste formula and whether negative waste is allowed. (QB-05-049,
  QB-05-050)
- QB-U-03 Whether container counts round up before pricing. (QB-05-093)
- QB-U-04 Order of escalation and tax relative to overhead and profit inside a section.
  (QB-08-054)
- QB-U-05 Selling price back-solve and rounding of the profit percent. (QB-11-009)
- QB-U-06 Indirect expense formulas when a material method posts a labor result.
  (QB-08-040)
- QB-U-07 Whether supervision cost receives burden and tax. (QB-08-049)
- QB-U-08 How the composite labor burden percent is weighted across payroll classes.
  (QB-10-038)
- QB-U-09 Payroll class hourly and daily wage formula from base, fees, insurance, tax.
  (QB-04-025)
- QB-U-10 "Repeat every" in the quantity math. (QB-05-055)
- QB-U-11 Mixture component quantity math. (QB-05-088)
- QB-U-12 Cut size production math. (QB-07-065)
- QB-U-13 Schedule of values O and P and percent-of-contract relations. (QB-13-015)
- QB-U-14 Which all-sections versus per-section indirect defaults win. (QB-03-025)
- QB-U-15 Takeoff file decimal rounding rule and when it is applied. (QB-06-037)

## Phase 3 build mapping (bidding module)

Phase 3a, the core JC needs to price a drywall and framing bid:
- Items master (material, labor, both; UOM; cost codes; sizes; containers), bid-level copy
  on first use. (sections 2, 5)
- Item quantity math: standard formula with installation methods, quantity x height,
  default to condition quantity, direct entry with ratio, OC exceptions, waste, labor on
  waste. (section 4)
- Material pricing per UOM and per container; labor with payroll classes, wage types,
  burdens, crews and production. (sections 6, 7)
- Condition detail grid with line totals; assemblies (store production and ratios).
  (section 8)
- Subs, equipment, other. (section 10)
- Sections with indirects (stock, clean, supervision, per diem), markups (straight and
  gross), additional markups, selling price back-solve. (section 11)
- Bid areas and typical areas shared with takeoff. (section 9)
- Reports: bid summary, job cost summary, cost percentages, labor risk. Excel export of
  everything. (section 14)

Phase 3b: alternates and change orders pricing rules, schedule of values, proposals,
material swap and container rounding, quantity survey import, takeoff file import.
Phase 4: accounting exports for Brightline's system, eQuotes, Athena bid estimate hookup.
