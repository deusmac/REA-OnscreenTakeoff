# QuickBid bid package (.qbp) format notes

Source: one of JC's own bid packages (copy only), read for file compatibility with his permission.
Everything here is our own description of what we observed. No vendor text, and no job data
(names, prices, notes) is recorded here. Reader code: `app/crates/quickbid`.

## Container

- A `.qbp` is a Microsoft Cabinet (starts with the bytes `MSCF`) holding a single XML file.
- The XML is UTF-8 with Windows line endings. Newlines inside text attributes are character
  references (`&#xd;&#xa;`), so attribute values can contain CR LF.
- The package is flat: every row is an `Item` element whose attributes are the columns. No
  element carries text content.
- The sample is about 28 MB unpacked, almost all of it contact rows in the masters.

## Layout

    BidPackage            Version="1"
      DBSettings          BidName, DBRequiresAuth
      Bid                 one bid row as attributes
        <Table TableId="n">
          Item ...        one per row, attributes are the columns
      Masters             company-wide master data (skipped by our reader)

Each table element has a `TableId`, and every `Item` repeats it as `ItemTableId`. It looks
like the numeric id of the source database table (the Access `.mdb` uses the same table names).

## Ids and links

- Every row has a `UID` (an integer id as text, unique within a table). Foreign keys are
  columns named `<Thing>UID`; an empty string means "none".
- Foreign keys point at rows in this package or at masters (for example `SectionUID`,
  `WageTypeUID`, `CostCodeUID`, `SupplierUID`, `PayClassUID`, `ItemTypeUID`). Masters targets
  are not resolvable by our reader because it does not load masters.

## Units and formats

- Lengths appear to be stored in tenths of a millimetre: a height of 33528 matches 11 ft,
  an on-center of 3048 matches 12 in. Confirm against more jobs.
- Dates are text like `dd/mm/yyyy h:mm:ss am`, with a `12:00:00 am` placeholder when unset.
  The reader keeps them as strings.
- Booleans are `0` or `-1`/`1`. Money and quantities are decimal text, sometimes with many
  digits (the bid total has about 12 decimals).
- Enumerations (`UOM1`, `RecType`, `QtyMethod`, `CalcMethod`, `SizeBy`, `MatPriceBy`,
  `LabPriceBy`, `Type`, `TotalView`) are small integers. Their labels are not in the package.

## Tables used by the reader (row counts from the sample)

- `Bid` (1 element, columns as attributes): `UID`, `BidNo`, `JobName`, `BidTotal`,
  `ExtrasTotal`, `BidDateTime`, `MarkupMethod`, `MeasureBase`, `HoursPerDay`, `DaysPerWeek`,
  `IsAccepted`, `Notes`, and many setup flags (`UseStock`, `UseClean`, `UseSupervision`,
  `UsePerDiem`, `ApplyLaborToWaste`, `OCSize1`, `OCSize2`). `OSTFilePath` records where the
  takeoff file lived. Job-site contact columns (phone, fax, contact) are dropped by the reader.
- `BidConditions` (23): a condition, the assembly that quantities are measured against.
  `Name`, `RefNo`, `MatTotal` and `LabTotal` (the totals QuickBid stored), `Height`, `Width`,
  `Depth`, `UOM1..UOM3` (units for three quantity slots), `Notes` (free text describing the
  build-up), `CdnTypeUID` (condition type), `IsTemplate`, `BidAlternateUID`.
- `BidAreaConditions` (23): quantity of a condition inside a bid area: `BidAreaUID`,
  `BidConditionUID`, `BidTypAreaUID`, `Count`, `Quantity1..3`. The area rows themselves are
  not in this package, so area names are unknown.
- `BidItems` (78): a priced line shared by conditions. `Code`, `Description`, `RecType`
  (probably material vs labor vs both), `UOM`, `Waste`, `QtyMethod`, `CalcMethod`,
  `OnCenter` and `OnCenterUOM` (spacing), `Layers`, `Height`, `SizeBy`, `MatPrice` with
  `MatPriceBy` and `MatPricePer` (price basis, for example per 1000), `LabPriceBy`, `LabProd`
  (labor productivity), `SectionUID`, `ContainerUID`, `ItemTypeUID`.
- `BidItemSizes` (309): size variants of an item with their own `Name`, `Width`, `Depth`,
  `Length`, `MatPrice`, `LabProd`, `Waste`, `Weight`, `UnitsPerCtr`, `SupplierUID`,
  `ProductNumber`, `Sequence`.
- `BidDetails` (230): the join between a condition and an item. `BidConditionUID`,
  `BidItemUID`, optional `BidItemSizeUID` and `BidMarkupUID`, plus the quantity rule for that
  pairing: `Quantity`, `CdnQty`, `QtyMethod`, `LabProd`, `Layers`, `OnCenter`, `Height`,
  `Sequence`. Per-detail values override the item defaults when present.
- `BidMarkups` (11): per-section markup percentages. Columns follow the pattern
  `<Category><Escalation|Overhead|Profit|Tax>Pct` for Mat, Lab, Equip, Subs, Other, plus
  method/result pairs for stock, clean and supervision. The reader keeps them in `extras`.
- `BidWages` (60): wage rates per pay class: `BaseWage` and `Burden1..Burden8`, with
  `PayClassUID` and `WageTypeUID`.
- `BidEquipment` (1) and `BidOthers` (3): same columns: `Description`, `Quantity`, `Rate`,
  `LabRate`, `Total`, `LabTotal`, `ApplyMarkup`, `Taxable`, `SectionUID`, `BidMarkupUID`,
  `BidWageUID`.
- Kept as raw rows by table name: `BidItemChains` (item to follow-on item), `BidItemInstallers`
  (crew per item), `BidItemMixtures` (mix components with waste), `BidItemSuppliers`,
  `BidQuoteRequests`, `BidQuoteRequestSuppliers`, `BidMUCleans`, `BidMUStocks`,
  `BidAreaEquipment`, `BidAreaOthers`.

## How the pieces link

    Bid
     +- BidConditions (BidUID)
     |    +- BidAreaConditions (BidConditionUID)  quantities per area
     |    +- BidDetails (BidConditionUID)
     |         +- BidItems (BidItemUID)           what is bought or installed
     |         +- BidItemSizes (BidItemSizeUID)   optional size of that item
     |         +- BidMarkups (BidMarkupUID)       optional markup row
     +- BidItems (BidUID) -> BidItemSizes (BidItemUID), Chains, Installers, Mixtures, Suppliers
     +- BidWages, BidEquipment, BidOthers, BidMarkups (BidUID)

A condition's stored `MatTotal` and `LabTotal` should be the sum over its details of
quantity times price (material) and quantity times productivity times wage (labor), with
waste, markup and burden applied somewhere in the chain. That is the check our pricing math
must reproduce; `summarize()` exposes the stored numbers for it.

## Privacy rules in the reader

- `Masters` is skipped whole. In the sample it holds about 150,000 contact rows and
  other master tables with names, phones, emails and addresses.
- Any attribute named like a person or contact field (name, phone, fax, email, address,
  login, password, site contact) is removed from every row, including extras.
- Never commit real job packages or output that prints job text. Tests use invented data.

## Open questions

1. Exact formula from details to `MatTotal`/`LabTotal` (waste on top of price or quantity,
   where `LabProd` units come from, how burden columns combine into a rate).
2. Meaning of each enumeration integer and where the labels live (probably masters).
3. Whether lengths are always tenths of a millimetre, or the unit follows a job setting.
4. How `Bid.BidTotal` is built from conditions, markups, equipment, others, stock, clean,
   supervision and per-diem (the `BidMU*` tables look like the last four).
5. Bid area and alternate tables (`BidAreaUID`, `BidAlternateUID` targets) are not in the
   package; check whether other jobs include them.
6. Which masters (cost codes, sections, wage types, units) a faithful reader needs, and
   how to load only their non-personal columns safely.
7. Whether multi-bid packages exist (one `Bid` element is assumed; the reader keeps the last).
8. The `.mdb` form of the same data uses identical table names; reading it is out of scope here.
