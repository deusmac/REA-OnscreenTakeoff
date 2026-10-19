//! Typed rows. Empty or missing numbers are `None`; empty UIDs are empty strings.
//! Anything not given a field lands in `extras` so no non-personal data is lost.
//! Lengths are stored by QuickBid in tenths of a millimetre (see the format notes).

use crate::attrs::Attrs;
use std::collections::BTreeMap;

pub type Extras = BTreeMap<String, String>;

/// One whole package.
#[derive(Debug, Default, Clone)]
pub struct BidPackage {
    pub version: String,
    /// `DBSettings/@BidName` (the bid's display name).
    pub db_bid_name: String,
    pub bid: Bid,
    pub conditions: Vec<Condition>,
    pub area_conditions: Vec<AreaCondition>,
    pub items: Vec<BidItem>,
    pub item_sizes: Vec<ItemSize>,
    pub details: Vec<Detail>,
    pub markups: Vec<Markup>,
    pub wages: Vec<Wage>,
    pub equipment: Vec<Charge>,
    pub others: Vec<Charge>,
    /// Every other bid table (chains, installers, mixtures, suppliers, quote requests,
    /// markup cleans/stocks, area equipment/others) as raw attribute rows by table name.
    pub other_tables: BTreeMap<String, Vec<Extras>>,
    /// True when a `Masters` section was present and skipped.
    pub masters_skipped: bool,
}

#[derive(Debug, Default, Clone)]
pub struct Bid {
    pub uid: String,
    pub bid_no: Option<i64>,
    pub job_name: String,
    pub job_id: String,
    pub bid_total: Option<f64>,
    pub extras_total: Option<f64>,
    pub bid_date_time: String,
    pub markup_method: Option<i64>,
    pub measure_base: Option<i64>,
    pub hours_per_day: Option<f64>,
    pub days_per_week: Option<f64>,
    pub is_accepted: bool,
    pub notes: String,
    pub extras: Extras,
}

/// A condition (assembly): the thing quantities are measured against.
#[derive(Debug, Default, Clone)]
pub struct Condition {
    pub uid: String,
    pub bid_uid: String,
    pub name: String,
    pub ref_no: Option<i64>,
    /// Stored material total as QuickBid last calculated it.
    pub mat_total: Option<f64>,
    /// Stored labor total as QuickBid last calculated it.
    pub lab_total: Option<f64>,
    pub height: Option<f64>,
    pub width: Option<f64>,
    pub depth: Option<f64>,
    /// Unit-of-measure codes for the three quantity slots (enum ids, meaning in masters).
    pub uom: [Option<i64>; 3],
    pub cdn_type_uid: String,
    pub alternate_uid: String,
    pub is_template: bool,
    pub notes: String,
    pub extras: Extras,
}

/// Quantity of a condition within a bid area.
#[derive(Debug, Default, Clone)]
pub struct AreaCondition {
    pub uid: String,
    pub bid_area_uid: String,
    pub bid_condition_uid: String,
    pub bid_typ_area_uid: String,
    pub count: Option<f64>,
    pub quantity: [Option<f64>; 3],
    pub extras: Extras,
}

/// A bid item: a priced material or labor line shared by conditions.
#[derive(Debug, Default, Clone)]
pub struct BidItem {
    pub uid: String,
    pub code: String,
    pub description: String,
    pub rec_type: Option<i64>,
    pub uom: Option<i64>,
    pub waste: Option<f64>,
    pub qty_method: Option<i64>,
    pub calc_method: Option<i64>,
    pub on_center: Option<f64>,
    pub on_center_uom: Option<i64>,
    pub layers: Option<f64>,
    pub height: Option<f64>,
    pub size_by: Option<i64>,
    pub mat_price: Option<f64>,
    pub mat_price_by: Option<i64>,
    pub mat_price_per: Option<f64>,
    pub lab_price_by: Option<i64>,
    pub lab_prod: Option<f64>,
    pub section_uid: String,
    pub container_uid: String,
    pub item_type_uid: String,
    pub extras: Extras,
}

/// A size variant of a bid item (own price, productivity and dimensions).
#[derive(Debug, Default, Clone)]
pub struct ItemSize {
    pub uid: String,
    pub bid_item_uid: String,
    pub name: String,
    pub sequence: Option<i64>,
    pub width: Option<f64>,
    pub depth: Option<f64>,
    pub length: Option<f64>,
    pub mat_price: Option<f64>,
    pub lab_prod: Option<f64>,
    pub waste: Option<f64>,
    pub weight: Option<f64>,
    pub units_per_ctr: Option<f64>,
    pub supplier_uid: String,
    pub product_number: String,
    pub extras: Extras,
}

/// Links a condition to an item (and optionally a size) with a quantity rule.
#[derive(Debug, Default, Clone)]
pub struct Detail {
    pub uid: String,
    pub bid_condition_uid: String,
    pub bid_item_uid: String,
    pub bid_item_size_uid: String,
    pub bid_markup_uid: String,
    pub quantity: Option<f64>,
    pub cdn_qty: Option<f64>,
    pub qty_method: Option<i64>,
    pub lab_prod: Option<f64>,
    pub layers: Option<f64>,
    pub on_center: Option<f64>,
    pub height: Option<f64>,
    pub sequence: Option<i64>,
    pub section_uid: String,
    pub extras: Extras,
}

/// Markup percentages for a section. The 20-odd percentage columns stay in `extras`
/// under their original names (for example `MatProfitPct`).
#[derive(Debug, Default, Clone)]
pub struct Markup {
    pub uid: String,
    pub section_uid: String,
    pub markup_method: Option<i64>,
    pub extras: Extras,
}

#[derive(Debug, Default, Clone)]
pub struct Wage {
    pub uid: String,
    pub pay_class_uid: String,
    pub wage_type_uid: String,
    pub base_wage: Option<f64>,
    pub burden: [Option<f64>; 8],
    pub extras: Extras,
}

/// Equipment or other-cost line (both tables share the same columns).
#[derive(Debug, Default, Clone)]
pub struct Charge {
    pub uid: String,
    pub description: String,
    pub quantity: Option<f64>,
    pub rate: Option<f64>,
    pub lab_rate: Option<f64>,
    pub total: Option<f64>,
    pub lab_total: Option<f64>,
    pub apply_markup: bool,
    pub taxable: bool,
    pub sequence: Option<i64>,
    pub section_uid: String,
    pub bid_markup_uid: String,
    pub bid_wage_uid: String,
    pub extras: Extras,
}

impl BidPackage {
    pub fn details_for<'a>(&'a self, condition_uid: &'a str) -> impl Iterator<Item = &'a Detail> {
        self.details
            .iter()
            .filter(move |d| d.bid_condition_uid == condition_uid)
    }
    pub fn item(&self, uid: &str) -> Option<&BidItem> {
        self.items.iter().find(|i| i.uid == uid)
    }
    pub fn size(&self, uid: &str) -> Option<&ItemSize> {
        self.item_sizes.iter().find(|s| s.uid == uid)
    }
}

impl Bid {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        Bid {
            uid: a.s("UID"),
            bid_no: a.i("BidNo"),
            job_name: a.s("JobName"),
            job_id: a.s("JobID"),
            bid_total: a.f("BidTotal"),
            extras_total: a.f("ExtrasTotal"),
            bid_date_time: a.s("BidDateTime"),
            markup_method: a.i("MarkupMethod"),
            measure_base: a.i("MeasureBase"),
            hours_per_day: a.f("HoursPerDay"),
            days_per_week: a.f("DaysPerWeek"),
            is_accepted: a.b("IsAccepted"),
            notes: a.s("Notes"),
            extras: a.rest(),
        }
    }
}

impl Condition {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        Condition {
            uid: a.s("UID"),
            bid_uid: a.s("BidUID"),
            name: a.s("Name"),
            ref_no: a.i("RefNo"),
            mat_total: a.f("MatTotal"),
            lab_total: a.f("LabTotal"),
            height: a.f("Height"),
            width: a.f("Width"),
            depth: a.f("Depth"),
            uom: [a.i("UOM1"), a.i("UOM2"), a.i("UOM3")],
            cdn_type_uid: a.s("CdnTypeUID"),
            alternate_uid: a.s("BidAlternateUID"),
            is_template: a.b("IsTemplate"),
            notes: a.s("Notes"),
            extras: a.rest(),
        }
    }
}

impl AreaCondition {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        AreaCondition {
            uid: a.s("UID"),
            bid_area_uid: a.s("BidAreaUID"),
            bid_condition_uid: a.s("BidConditionUID"),
            bid_typ_area_uid: a.s("BidTypAreaUID"),
            count: a.f("Count"),
            quantity: [a.f("Quantity1"), a.f("Quantity2"), a.f("Quantity3")],
            extras: a.rest(),
        }
    }
}

impl BidItem {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        BidItem {
            uid: a.s("UID"),
            code: a.s("Code"),
            description: a.s("Description"),
            rec_type: a.i("RecType"),
            uom: a.i("UOM"),
            waste: a.f("Waste"),
            qty_method: a.i("QtyMethod"),
            calc_method: a.i("CalcMethod"),
            on_center: a.f("OnCenter"),
            on_center_uom: a.i("OnCenterUOM"),
            layers: a.f("Layers"),
            height: a.f("Height"),
            size_by: a.i("SizeBy"),
            mat_price: a.f("MatPrice"),
            mat_price_by: a.i("MatPriceBy"),
            mat_price_per: a.f("MatPricePer"),
            lab_price_by: a.i("LabPriceBy"),
            lab_prod: a.f("LabProd"),
            section_uid: a.s("SectionUID"),
            container_uid: a.s("ContainerUID"),
            item_type_uid: a.s("ItemTypeUID"),
            extras: a.rest(),
        }
    }
}

impl ItemSize {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        ItemSize {
            uid: a.s("UID"),
            bid_item_uid: a.s("BidItemUID"),
            name: a.s("Name"),
            sequence: a.i("Sequence"),
            width: a.f("Width"),
            depth: a.f("Depth"),
            length: a.f("Length"),
            mat_price: a.f("MatPrice"),
            lab_prod: a.f("LabProd"),
            waste: a.f("Waste"),
            weight: a.f("Weight"),
            units_per_ctr: a.f("UnitsPerCtr"),
            supplier_uid: a.s("SupplierUID"),
            product_number: a.s("ProductNumber"),
            extras: a.rest(),
        }
    }
}

impl Detail {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        Detail {
            uid: a.s("UID"),
            bid_condition_uid: a.s("BidConditionUID"),
            bid_item_uid: a.s("BidItemUID"),
            bid_item_size_uid: a.s("BidItemSizeUID"),
            bid_markup_uid: a.s("BidMarkupUID"),
            quantity: a.f("Quantity"),
            cdn_qty: a.f("CdnQty"),
            qty_method: a.i("QtyMethod"),
            lab_prod: a.f("LabProd"),
            layers: a.f("Layers"),
            on_center: a.f("OnCenter"),
            height: a.f("Height"),
            sequence: a.i("Sequence"),
            section_uid: a.s("SectionUID"),
            extras: a.rest(),
        }
    }
}

impl Markup {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        Markup {
            uid: a.s("UID"),
            section_uid: a.s("SectionUID"),
            markup_method: a.i("MarkupMethod"),
            extras: a.rest(),
        }
    }
}

impl Wage {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        Wage {
            uid: a.s("UID"),
            pay_class_uid: a.s("PayClassUID"),
            wage_type_uid: a.s("WageTypeUID"),
            base_wage: a.f("BaseWage"),
            burden: std::array::from_fn(|n| a.f(&format!("Burden{}", n + 1))),
            extras: a.rest(),
        }
    }
}

impl Charge {
    pub(crate) fn from_attrs(mut a: Attrs) -> Self {
        Charge {
            uid: a.s("UID"),
            description: a.s("Description"),
            quantity: a.f("Quantity"),
            rate: a.f("Rate"),
            lab_rate: a.f("LabRate"),
            total: a.f("Total"),
            lab_total: a.f("LabTotal"),
            apply_markup: a.b("ApplyMarkup"),
            taxable: a.b("Taxable"),
            sequence: a.i("Sequence"),
            section_uid: a.s("SectionUID"),
            bid_markup_uid: a.s("BidMarkupUID"),
            bid_wage_uid: a.s("BidWageUID"),
            extras: a.rest(),
        }
    }
}
