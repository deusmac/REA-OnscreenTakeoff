//! Import of an On-Screen Takeoff project package (`.osp`) or bare job file (`.ost`) into
//! the `rea_core::model` types. Format notes: `docs/study/formats/ost-package.md`.
//!
//! Read only. Nothing is written to disk: plan PDFs come back as bytes for the caller to
//! save. The `Employees` section (which holds credentials) is never read.

mod xml;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::str::FromStr;

use rea_core::model::{
    BidArea, Condition, ConditionStyle, Geometry, Id, Measure, Page, Point, Project, ResultKind,
    Scale, ScaleStyle, Takeoff,
};
use rea_core::ost_totals::{OstCondition, OstJob, OstTakeoff, TypAreaCount, ZoneOutline};
use thiserror::Error;
use xml::Node;

/// Takeoff coordinates are real-world inches, already scaled (docs/study/formats/ost-quantities.md,
/// confirmed by 1962 of 1971 OST totals); each page's scale turns them back into sheet points.
const POINTS_PER_INCH: f64 = 72.0;
const MAX_JOB_FILE: u64 = 512 * 1024 * 1024;
const MAX_PLAN_FILE: u64 = 2 * 1024 * 1024 * 1024;
/// Plan files sit under this folder inside the archive, mirroring the original path.
const PLAN_ROOT: &str = "TempImages!.tmp";

#[derive(Debug, Error)]
pub enum OstError {
    #[error("read error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML error: {0}")]
    Xml(String),
    #[error("package has no .ost job file")]
    NoJobFile,
    #[error("package has more than one .ost job file")]
    ManyJobFiles,
    #[error("file {name} is too large ({size} bytes)")]
    TooLarge { name: String, size: u64 },
    #[error("job file has no <{0}> element")]
    MissingElement(&'static str),
    #[error("{entity} has no usable UID")]
    MissingUid { entity: &'static str },
    #[error("this file is part {part} of a package split across several files; open the set from its first file")]
    SplitPackage { part: u16 },
    #[error("{entity} {uid}: bad value {value:?} for {attr}")]
    BadValue {
        entity: &'static str,
        uid: String,
        attr: &'static str,
        value: String,
    },
    #[error("{entity} {uid}: {attr} refers to unknown {target} {value}")]
    UnknownRef {
        entity: &'static str,
        uid: String,
        attr: &'static str,
        target: &'static str,
        value: String,
    },
}

/// A plan file found inside the package.
#[derive(Debug, Clone)]
pub struct PlanFile {
    /// Path inside the archive, with `\` separators as stored.
    pub archive_path: String,
    pub bytes: Vec<u8>,
}

/// Attributes we do not map, kept verbatim (string form) so nothing is dropped silently.
/// Coded fields we only partly understand (Type, Shape, Pattern, UOM1-3, Quantity1-3,
/// ScaleStyle) also stay here in raw form.
pub type Extra = BTreeMap<String, String>;

/// Arc data of a curved linear segment, in page points. Our geometry has no arcs yet, so the
/// polyline holds the chord and the arc rides here.
#[derive(Debug, Clone, PartialEq)]
pub struct Arc {
    /// A third point on the arc (as stored by OST).
    pub through: Point,
    /// Signed bulge value as stored, converted to points.
    pub sagitta: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TakeoffRaw {
    pub arc: Option<Arc>,
    /// `Position` numbers as stored (real inches), for `rea_core::ost_totals`.
    pub position: Vec<f64>,
    pub extra: Extra,
}

/// One bid (the main bid or an alternate) mapped into our model.
#[derive(Debug, Clone)]
pub struct OstBid {
    pub project: Project,
    pub project_extra: Extra,
    pub pages: Vec<Page>,
    pub page_extra: HashMap<Id, Extra>,
    pub conditions: Vec<Condition>,
    pub condition_extra: HashMap<Id, Extra>,
    pub takeoffs: Vec<Takeoff>,
    pub takeoff_raw: HashMap<Id, TakeoffRaw>,
    pub bid_areas: Vec<BidArea>,
    /// Zone regions: takeoffs with a zone and no condition. Not seen in any package yet.
    pub zone_outlines: Vec<ZoneOutline>,
    /// `BidZone` elements marked `IsNegativeValues`.
    pub negative_zones: HashSet<Id>,
    /// `BidTypAreaCount` elements.
    pub typ_area_counts: Vec<TypAreaCount>,
}

#[derive(Debug, Clone)]
pub struct OstImport {
    /// Version string of the writing application, as stored.
    pub ost_version: String,
    pub bid: OstBid,
    pub alternates: Vec<OstBid>,
    /// Plan PDFs and other non-job files from the package (empty for a bare `.ost`).
    pub plan_files: Vec<PlanFile>,
}

/// Archive path a page's `file_path` (the original absolute path) is stored under in the
/// package: the drive prefix is dropped and the rest sits below the plan root folder.
pub fn plan_archive_path(image_path: &str) -> String {
    let rest = match image_path.as_bytes() {
        [d, b':', b'\\' | b'/', ..] if d.is_ascii_alphabetic() => &image_path[3..],
        _ => image_path,
    };
    format!("{PLAN_ROOT}\\{}", rest.replace('/', "\\"))
}

/// Import from file bytes: a cabinet (`MSCF`) package or a bare XML job file.
pub fn import_bytes(data: &[u8]) -> Result<OstImport, OstError> {
    if data.starts_with(b"MSCF") {
        import_package(data)
    } else {
        Ok(import_ost(data, Vec::new())?)
    }
}

pub fn import_path(path: &std::path::Path) -> Result<OstImport, OstError> {
    import_bytes(&std::fs::read(path)?)
}

fn import_package(data: &[u8]) -> Result<OstImport, OstError> {
    // CAB header flags (offset 30): 1 = has a previous cabinet, 2 = has a next one. Spanned
    // sets are not supported yet; seen once (a 46 MB job whose first part was missing).
    if data.len() >= 36 && u16::from_le_bytes([data[30], data[31]]) & 3 != 0 {
        let part = u16::from_le_bytes([data[34], data[35]]) + 1;
        return Err(OstError::SplitPackage { part });
    }
    let mut cabinet = cab::Cabinet::new(Cursor::new(data))?;
    let mut entries: Vec<(String, u64)> = Vec::new();
    for folder in cabinet.folder_entries() {
        for f in folder.file_entries() {
            entries.push((f.name().to_string(), u64::from(f.uncompressed_size())));
        }
    }
    let is_job = |n: &str| n.to_ascii_lowercase().ends_with(".ost");
    let mut jobs = entries.iter().filter(|(n, _)| is_job(n));
    let (job_name, _) = jobs.next().ok_or(OstError::NoJobFile)?.clone();
    if jobs.next().is_some() {
        return Err(OstError::ManyJobFiles);
    }
    let mut job = Vec::new();
    let mut plans = Vec::new();
    for (name, size) in &entries {
        let is_job_file = *name == job_name;
        let is_plan = !is_job_file && name.starts_with(PLAN_ROOT);
        if !is_job_file && !is_plan {
            continue; // BidTrans.xml and anything else: not needed to rebuild the job
        }
        let limit = if is_job_file {
            MAX_JOB_FILE
        } else {
            MAX_PLAN_FILE
        };
        if *size > limit {
            return Err(OstError::TooLarge {
                name: name.clone(),
                size: *size,
            });
        }
        let mut bytes = Vec::with_capacity(*size as usize);
        cabinet.read_file(name)?.read_to_end(&mut bytes)?;
        if is_job_file {
            job = bytes;
        } else {
            plans.push(PlanFile {
                archive_path: name.clone(),
                bytes,
            });
        }
    }
    import_ost(&job, plans)
}

fn import_ost(data: &[u8], plan_files: Vec<PlanFile>) -> Result<OstImport, OstError> {
    let root = xml::parse(data)?;
    let bid_node = root.child("Bid").ok_or(OstError::MissingElement("Bid"))?;
    let cdn_types: HashMap<String, String> = root
        .child("CdnTypes")
        .map(|n| {
            n.children_named("CdnType")
                .map(|c| (attr(c, "UID"), attr(c, "Name")))
                .collect()
        })
        .unwrap_or_default();
    let bid = map_bid(bid_node, &cdn_types)?;
    let alternates = bid_node
        .child("ALTERNATES")
        .map(|n| {
            n.children_named("Bid")
                .map(|b| map_bid(b, &cdn_types))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let ost_version = root
        .child("OST")
        .map(|n| attr(n, "Version"))
        .unwrap_or_default();
    Ok(OstImport {
        ost_version,
        bid,
        alternates,
        plan_files,
    })
}

// ---- attribute helpers -------------------------------------------------------------------

fn attr(n: &Node, key: &str) -> String {
    n.attrs.get(key).cloned().unwrap_or_default()
}

fn take_str(m: &mut Extra, key: &str) -> String {
    m.remove(key).unwrap_or_default()
}

/// Numeric attribute: missing or empty is the default; a non-number is an error.
fn take_num<T: FromStr + Default>(
    m: &mut Extra,
    entity: &'static str,
    uid: &str,
    key: &'static str,
) -> Result<T, OstError> {
    match m.remove(key) {
        // Some jobs store the text NULL for an unset number (seen on page Rotation).
        Some(v) if !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("NULL") => {
            v.trim().parse().map_err(|_| OstError::BadValue {
                entity,
                uid: uid.to_string(),
                attr: key,
                value: v,
            })
        }
        _ => Ok(T::default()),
    }
}

fn take_f64(
    m: &mut Extra,
    entity: &'static str,
    uid: &str,
    key: &'static str,
) -> Result<f64, OstError> {
    let v: f64 = take_num(m, entity, uid, key)?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(OstError::BadValue {
            entity,
            uid: uid.to_string(),
            attr: key,
            value: v.to_string(),
        })
    }
}

fn take_uid(m: &mut Extra, entity: &'static str) -> Result<Id, OstError> {
    m.remove("UID")
        .and_then(|v| v.trim().parse::<Id>().ok())
        .ok_or(OstError::MissingUid { entity })
}

fn bad(entity: &'static str, uid: &str, attr: &'static str, value: &str) -> OstError {
    OstError::BadValue {
        entity,
        uid: uid.to_string(),
        attr,
        value: value.to_string(),
    }
}

/// OST writes dates as "2026 5 13 0 10 47".
fn iso_date(s: &str) -> Option<String> {
    let n: Vec<u32> = s
        .split_whitespace()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match n[..] {
        [y, mo, d, h, mi, se]
            if (1..=12).contains(&mo) && (1..=31).contains(&d) && h < 24 && mi < 60 && se < 60 =>
        {
            Some(format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}"))
        }
        _ => None,
    }
}

/// Windows color value (blue in the high byte) to RGB.
fn rgb(v: i64) -> [u8; 3] {
    [
        (v & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        ((v >> 16) & 0xFF) as u8,
    ]
}

// ---- mapping -----------------------------------------------------------------------------

fn map_bid(n: &Node, cdn_types: &HashMap<String, String>) -> Result<OstBid, OstError> {
    let mut m = n.attrs.clone();
    let uid = take_uid(&mut m, "Bid")?;
    let uid_s = uid.to_string();
    let created_raw = take_str(&mut m, "CreateDateTime");
    let created = if created_raw.trim().is_empty() {
        String::new()
    } else {
        iso_date(&created_raw).ok_or_else(|| bad("Bid", &uid_s, "CreateDateTime", &created_raw))?
    };
    let source = take_str(&mut m, "SourceBidUID");
    // MeasureBase: 0 observed (imperial). 1 for metric is assumed, not yet seen.
    let measure = if m.get("MeasureBase").map(|v| v.trim()) == Some("1") {
        Measure::Metric
    } else {
        Measure::Imperial
    };
    let project = Project {
        id: uid,
        name: take_str(&mut m, "JobName"),
        bid_no: take_str(&mut m, "BidNo"),
        job_no: take_str(&mut m, "JobID"),
        estimator: String::new(), // only a UID here; names live in the skipped Employees list
        notes: take_str(&mut m, "Notes"),
        measure,
        created,
        origin_id: (!source.trim().is_empty()).then_some(source),
    };

    let layers: HashMap<String, String> = n
        .child("BidLayers")
        .map(|l| {
            l.children_named("BidLayer")
                .map(|x| (attr(x, "UID"), attr(x, "Name")))
                .collect()
        })
        .unwrap_or_default();

    let mut pages = Vec::new();
    let mut page_extra = HashMap::new();
    let mut takeoff_nodes: Vec<(Id, &Node)> = Vec::new();
    if let Some(bp) = n.child("BidPages") {
        for (i, pn) in bp.children_named("BidPage").enumerate() {
            let (page, extra) = map_page(pn, uid, i as u32)?;
            if let Some(t) = pn.child("BidTakeoffs") {
                takeoff_nodes.extend(t.children_named("BidTakeoff").map(|x| (page.id, x)));
            }
            page_extra.insert(page.id, extra);
            pages.push(page);
        }
    }

    let mut conditions = Vec::new();
    let mut condition_extra = HashMap::new();
    if let Some(bc) = n.child("BidConditions") {
        for cn in bc.children_named("BidCondition") {
            let (c, extra) = map_condition(cn, uid, &layers, cdn_types)?;
            condition_extra.insert(c.id, extra);
            conditions.push(c);
        }
    }

    // Area names are not seen in any sample yet (the section was empty); attribute names
    // below are a guess, see the format notes.
    let mut bid_areas = Vec::new();
    if let Some(ba) = n.child("BidAreas") {
        for (order, an) in descendants_named(ba, "BidArea").enumerate() {
            let mut am = an.attrs.clone();
            let id = take_uid(&mut am, "BidArea")?;
            let parent = take_str(&mut am, "ParentUID")
                .trim()
                .parse::<Id>()
                .ok()
                .filter(|p| *p != 0);
            bid_areas.push(BidArea {
                id,
                project_id: uid,
                name: take_str(&mut am, "Name"),
                parent_id: parent,
                order: order as i64,
            });
        }
    }

    let cond_style: HashMap<Id, ConditionStyle> =
        conditions.iter().map(|c| (c.id, c.style)).collect();
    let mut takeoffs = Vec::new();
    let mut takeoff_raw = HashMap::new();
    // A page without a scale keeps real inches as points; OST does not allow takeoff there.
    let page_k: HashMap<Id, f64> = pages
        .iter()
        .map(|p| {
            (
                p.id,
                p.scale.as_ref().map_or(1.0, |s| 1.0 / s.real_per_page_unit),
            )
        })
        .collect();
    let mut zone_outlines = Vec::new();
    for (page_id, tn) in takeoff_nodes {
        // A zone region is a takeoff with a zone and no condition (seen in the job database,
        // not yet in a package).
        let no_cond = matches!(attr(tn, "BidConditionUID").trim(), "" | "0");
        let zone = attr(tn, "BidZoneUID").trim().parse::<Id>().unwrap_or(0);
        if no_cond && zone != 0 {
            let u = attr(tn, "UID");
            let v = parse_position(&attr(tn, "Position"), &u)?;
            zone_outlines.push(ZoneOutline {
                zone,
                page: page_id,
                ring: v.chunks_exact(2).map(|c| (c[0], c[1])).collect(),
            });
            continue;
        }
        let k = page_k.get(&page_id).copied().unwrap_or(1.0);
        let (t, raw) = map_takeoff(tn, page_id, k, &cond_style, &bid_areas)?;
        takeoff_raw.insert(t.id, raw);
        takeoffs.push(t);
    }
    // Element and attribute names follow the job database columns; both sections were empty
    // in every package read so far.
    let num_attr = |x: &Node, k: &str| attr(x, k).trim().parse::<f64>().unwrap_or(0.0);
    let negative_zones = descendants_named(n, "BidZone")
        .filter(|z| attr(z, "IsNegativeValues").trim() == "1")
        .filter_map(|z| attr(z, "UID").trim().parse().ok())
        .collect();
    let typ_area_counts = descendants_named(n, "BidTypAreaCount")
        .map(|x| TypAreaCount {
            area: num_attr(x, "BidAreaUID") as Id,
            typ_area: num_attr(x, "BidTypAreaUID") as Id,
            count: num_attr(x, "Count"),
        })
        .collect();

    Ok(OstBid {
        project,
        project_extra: m,
        pages,
        page_extra,
        conditions,
        condition_extra,
        takeoffs,
        takeoff_raw,
        bid_areas,
        zone_outlines,
        negative_zones,
        typ_area_counts,
    })
}

/// The bid as input for `rea_core::ost_totals`: OST's raw numbers and codes, positions in
/// real inches as stored.
pub fn totals_job(bid: &OstBid) -> OstJob {
    let no_extra = Extra::new();
    let conditions = bid
        .conditions
        .iter()
        .map(|c| {
            let m = bid.condition_extra.get(&c.id).unwrap_or(&no_extra);
            let s = |k: &str| m.get(k).map_or("", |v| v.trim());
            let f = |k: &str| s(k).parse::<f64>().unwrap_or(0.0);
            let (rise, run) = c.slope.unwrap_or((0.0, 0.0));
            OstCondition {
                uid: c.id,
                kind: s("Type").parse().unwrap_or(-1),
                shape: s("Shape").parse().unwrap_or(0),
                width: c.width,
                height: c.height,
                thickness: c.thickness,
                depth: c.depth,
                rise,
                run,
                results: std::array::from_fn(|i| {
                    s(&format!("Quantity{}", i + 1)).parse().unwrap_or(u32::MAX)
                }),
                uoms: std::array::from_fn(|i| s(&format!("UOM{}", i + 1)).parse().unwrap_or(-1)),
                round_quantity: s("RoundQuantity") == "1",
                round_up: f("RoundUp"),
                backout: s("Backout") == "1",
                drop_run: s("DropRun") == "1",
                grid_size: (f("GridSize1"), f("GridSize2")),
                gap: f("Gap"),
            }
        })
        .collect();
    let no_raw = TakeoffRaw::default();
    let takeoffs = bid
        .takeoffs
        .iter()
        .map(|t| {
            let raw = bid.takeoff_raw.get(&t.id).unwrap_or(&no_raw);
            let s = |k: &str| raw.extra.get(k).map_or("", |v| v.trim());
            let f = |k: &str| s(k).parse::<f64>().unwrap_or(0.0);
            let id = |k: &str| s(k).parse::<Id>().ok().filter(|v| *v != 0);
            OstTakeoff {
                uid: t.id,
                page: t.page_id,
                condition: t.condition_id,
                zone: id("BidZoneUID"),
                area: t.bid_area_id,
                typ_area: id("BidTypAreaUID"),
                parent: id("ParentUID"),
                typ_group: id("TypGroupUID"),
                position: raw.position.clone(),
                count: f("Count"),
                negative: t.negative,
                grid_offset: (f("GridOffsetX"), f("GridOffsetY")),
                grid_rotation: f("GridRotation"),
            }
        })
        .collect();
    OstJob {
        conditions,
        takeoffs,
        zone_outlines: bid.zone_outlines.clone(),
        negative_zones: bid.negative_zones.clone(),
        typ_area_counts: bid.typ_area_counts.clone(),
    }
}

fn descendants_named<'a>(n: &'a Node, name: &'a str) -> Box<dyn Iterator<Item = &'a Node> + 'a> {
    Box::new(n.children.iter().flat_map(move |c| {
        let own = (c.name == name).then_some(c);
        own.into_iter().chain(descendants_named(c, name))
    }))
}

fn map_page(n: &Node, project_id: Id, order: u32) -> Result<(Page, Extra), OstError> {
    let mut m = n.attrs.clone();
    let id = take_uid(&mut m, "BidPage")?;
    let u = id.to_string();
    let width_in = take_f64(&mut m, "BidPage", &u, "Width")?;
    let height_in = take_f64(&mut m, "BidPage", &u, "Height")?;
    let f1 = take_f64(&mut m, "BidPage", &u, "ScaleFactor1")?;
    let f2 = take_f64(&mut m, "BidPage", &u, "ScaleFactor2")?;
    let index1: u32 = take_num(&mut m, "BidPage", &u, "Index1")?;
    let rot: i64 = take_num(&mut m, "BidPage", &u, "Rotation")?;
    if rot % 90 != 0 {
        return Err(bad("BidPage", &u, "Rotation", &rot.to_string()));
    }
    // ScaleStyle stays in `extra` (peeked, not taken): only code 1 is understood.
    let custom = m.get("IsCustomScale").map(|v| v.trim()) == Some("1");
    let style = if !custom && m.get("ScaleStyle").map(|v| v.trim()) == Some("1") {
        ScaleStyle::Architectural
    } else {
        ScaleStyle::Custom
    };
    // Factor1 inches on paper stand for factor2 real inches. Our scale is per PDF point.
    let scale = (f1 > 0.0 && f2 > 0.0).then(|| Scale {
        real_per_page_unit: f2 / f1 / POINTS_PER_INCH,
        style,
        label: format!("{f1} in = {f2} in"),
    });
    let page = Page {
        id,
        project_id,
        index: order,
        sheet_no: take_str(&mut m, "SheetNo"),
        sheet_name: take_str(&mut m, "Name"),
        file_path: take_str(&mut m, "ImagePath"),
        file_page_index: index1.saturating_sub(1),
        width: width_in * POINTS_PER_INCH,
        height: height_in * POINTS_PER_INCH,
        scale,
        rotation: rot.rem_euclid(360) as u16,
        complete: false,
    };
    Ok((page, m))
}

fn map_condition(
    n: &Node,
    project_id: Id,
    layers: &HashMap<String, String>,
    cdn_types: &HashMap<String, String>,
) -> Result<(Condition, Extra), OstError> {
    let mut m = n.attrs.clone();
    let id = take_uid(&mut m, "BidCondition")?;
    let u = id.to_string();
    // Type, Quantity1-3, UOM1-3, Shape, Pattern stay in `extra` (peeked, not taken).
    let type_code = m
        .get("Type")
        .map(|v| v.trim().to_string())
        .unwrap_or_default();
    let style = match type_code.as_str() {
        "0" => ConditionStyle::Linear,
        "1" => ConditionStyle::Area,
        // 3 = attached count (door or window on a wall, ost-quantities.md). 4 is unknown: seen
        // only with count result codes and no takeoffs; kept as a count, raw code in `extra`.
        "2" | "3" | "4" => ConditionStyle::Count,
        other => return Err(bad("BidCondition", &u, "Type", other)),
    };
    let height = take_f64(&mut m, "BidCondition", &u, "Height")?;
    let thickness = take_f64(&mut m, "BidCondition", &u, "Thickness")?;
    let width = take_f64(&mut m, "BidCondition", &u, "Width")?;
    let depth = take_f64(&mut m, "BidCondition", &u, "Depth")?;
    let rise = take_f64(&mut m, "BidCondition", &u, "Rise")?;
    let run = take_f64(&mut m, "BidCondition", &u, "Run")?;
    let fill: i64 = take_num(&mut m, "BidCondition", &u, "ColorFill")?;
    let number_raw = take_str(&mut m, "RefNo");
    let number = if number_raw.trim().is_empty() {
        id
    } else {
        number_raw
            .trim()
            .parse()
            .map_err(|_| bad("BidCondition", &u, "RefNo", &number_raw))?
    };
    let mut results = [None; 3];
    for (slot, key) in results
        .iter_mut()
        .zip(["Quantity1", "Quantity2", "Quantity3"])
    {
        let code = m.get(key).map(|v| v.trim().to_string()).unwrap_or_default();
        *slot = result_kind(style, &code);
    }
    let lookup = |map: &HashMap<String, String>, key: &str, m: &Extra| {
        m.get(key)
            .and_then(|k| map.get(k.trim()))
            .cloned()
            .unwrap_or_default()
    };
    let c = Condition {
        id,
        project_id,
        number,
        name: take_str(&mut m, "Name"),
        style,
        ctype: lookup(cdn_types, "CdnTypeUID", &m),
        layer: lookup(layers, "BidLayerUID", &m),
        color: rgb(fill),
        pattern: m.get("Pattern").cloned().unwrap_or_default(),
        height,
        thickness,
        width,
        depth,
        slope: (rise != 0.0 || run != 0.0).then_some((rise, run)),
        count_shape: None, // Shape codes not decoded yet; raw code stays in `extra`
        results,
    };
    Ok((c, m))
}

/// Result-kind codes seen in a real job. Codes not listed stay `None` (raw code in `extra`).
/// Inferred from names and units, not confirmed against OST quantities: see the format notes.
fn result_kind(style: ConditionStyle, code: &str) -> Option<ResultKind> {
    match (style, code) {
        (ConditionStyle::Linear, "1") => Some(ResultKind::Length),
        (ConditionStyle::Area, "11") => Some(ResultKind::Area),
        (ConditionStyle::Area, "15") => Some(ResultKind::Perimeter),
        (ConditionStyle::Count, "23") => Some(ResultKind::Count),
        (ConditionStyle::Count, "24") => Some(ResultKind::TotalHeight),
        _ => None,
    }
}

fn parse_position(s: &str, uid: &str) -> Result<Vec<f64>, OstError> {
    // Some takeoffs end with a stray ';' before the newline.
    let t = s.trim().trim_end_matches(';');
    if t.is_empty() {
        return Err(bad("BidTakeoff", uid, "Position", s));
    }
    t.split(';')
        .map(|p| p.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| bad("BidTakeoff", uid, "Position", s))
}

/// `k` is sheet points per real inch for the takeoff's page.
fn points(flat: &[f64], k: f64) -> Vec<Point> {
    flat.chunks_exact(2)
        .map(|c| Point {
            x: c[0] * k,
            y: c[1] * k,
        })
        .collect()
}

fn map_takeoff(
    n: &Node,
    page_id: Id,
    k: f64,
    cond_style: &HashMap<Id, ConditionStyle>,
    areas: &[BidArea],
) -> Result<(Takeoff, TakeoffRaw), OstError> {
    let mut m = n.attrs.clone();
    let id = take_uid(&mut m, "BidTakeoff")?;
    let u = id.to_string();
    let cond_raw = take_str(&mut m, "BidConditionUID");
    let condition_id: Id = cond_raw
        .trim()
        .parse()
        .map_err(|_| bad("BidTakeoff", &u, "BidConditionUID", &cond_raw))?;
    let style = *cond_style
        .get(&condition_id)
        .ok_or_else(|| OstError::UnknownRef {
            entity: "BidTakeoff",
            uid: u.clone(),
            attr: "BidConditionUID",
            target: "condition",
            value: cond_raw.clone(),
        })?;
    let area_raw = take_str(&mut m, "BidAreaUID");
    let bid_area_id = match area_raw.trim() {
        "" | "0" => None,
        v => {
            let a: Id = v
                .parse()
                .map_err(|_| bad("BidTakeoff", &u, "BidAreaUID", v))?;
            if !areas.iter().any(|x| x.id == a) {
                return Err(OstError::UnknownRef {
                    entity: "BidTakeoff",
                    uid: u.clone(),
                    attr: "BidAreaUID",
                    target: "bid area",
                    value: v.to_string(),
                });
            }
            Some(a)
        }
    };
    // Only 0 has been seen; any other non-empty value is taken as negative.
    let negative = !matches!(take_str(&mut m, "IsNegativeQuantity").trim(), "" | "0");
    let pos_raw = take_str(&mut m, "Position");
    let nums = parse_position(&pos_raw, &u)?;
    let mut arc = None;
    let geometry = match style {
        ConditionStyle::Linear => {
            // Linear: points, then three values (arc point x, y and signed bulge), all zero
            // for a straight segment.
            // Older jobs write one segment as x1;y1;x2;y2;s (s = signed arc height).
            // shortcut: the arc's through point is set to the chord middle, since only s is stored;
            // compute the true point once the sign convention is pinned down.
            if nums.len() == 5 {
                let pts = &nums[..4];
                if nums[4] != 0.0 {
                    arc = Some(Arc {
                        through: Point {
                            x: (pts[0] + pts[2]) / 2.0 * k,
                            y: (pts[1] + pts[3]) / 2.0 * k,
                        },
                        sagitta: nums[4] * k,
                    });
                }
                return Ok((
                    Takeoff {
                        id,
                        page_id,
                        condition_id,
                        bid_area_id,
                        negative,
                        geometry: Geometry::Polyline(points(pts, k)),
                    },
                    TakeoffRaw {
                        arc,
                        position: nums.clone(),
                        extra: m,
                    },
                ));
            }
            let (pts, trailer) = if nums.len() % 2 == 1 {
                let (p, t) = nums.split_at(nums.len().saturating_sub(3));
                (p, Some(t))
            } else {
                (&nums[..], None)
            };
            if pts.len() < 4 || pts.len() % 2 == 1 {
                return Err(bad("BidTakeoff", &u, "Position", &pos_raw));
            }
            if let Some([x, y, s]) = trailer {
                if *x != 0.0 || *y != 0.0 || *s != 0.0 {
                    arc = Some(Arc {
                        through: Point {
                            x: *x * k,
                            y: *y * k,
                        },
                        sagitta: *s * k,
                    });
                }
            }
            Geometry::Polyline(points(pts, k))
        }
        ConditionStyle::Area => {
            if nums.len() < 6 || nums.len() % 2 == 1 {
                return Err(bad("BidTakeoff", &u, "Position", &pos_raw));
            }
            Geometry::Polygon {
                outer: points(&nums, k),
                backouts: Vec::new(),
            }
        }
        ConditionStyle::Count => match nums.len() {
            2 => Geometry::Point(points(&nums, k)[0]),
            // Attached count: distance along the parent wall and side, then the point.
            4 => {
                m.insert("AttachDistance".into(), nums[0].to_string());
                m.insert("AttachSide".into(), nums[1].to_string());
                Geometry::Point(points(&nums[2..], k)[0])
            }
            _ => return Err(bad("BidTakeoff", &u, "Position", &pos_raw)),
        },
    };
    let t = Takeoff {
        id,
        page_id,
        condition_id,
        bid_area_id,
        negative,
        geometry,
    };
    Ok((
        t,
        TakeoffRaw {
            arc,
            position: nums,
            extra: m,
        },
    ))
}

#[cfg(test)]
mod tests;
