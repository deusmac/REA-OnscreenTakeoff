//! Streaming parse: CAB bytes (or raw XML) to `BidPackage`.

use crate::attrs::Attrs;
use crate::model::*;
use crate::QbError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

/// Read a `.qbp` (a CAB with one XML file). Raw XML bytes are accepted too.
pub fn read_package(bytes: &[u8]) -> Result<BidPackage, QbError> {
    if !bytes.starts_with(b"MSCF") {
        return read_package_xml(bytes);
    }
    let mut cabinet = cab::Cabinet::new(Cursor::new(bytes))?;
    let name = cabinet
        .folder_entries()
        .flat_map(|f| f.file_entries())
        .map(|f| f.name().to_string())
        .next()
        .ok_or(QbError::EmptyCabinet)?;
    let mut xml = Vec::new();
    cabinet.read_file(&name)?.read_to_end(&mut xml)?;
    read_package_xml(&xml)
}

fn attrs_of(e: &BytesStart) -> Result<Attrs, QbError> {
    let mut map = BTreeMap::new();
    for a in e.attributes() {
        let a = a.map_err(|x| QbError::Xml(x.to_string()))?;
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let val = a
            .unescape_value()
            .map_err(|x| QbError::Xml(x.to_string()))?
            .into_owned();
        map.insert(key, val);
    }
    Ok(Attrs::new(map))
}

/// Parse the package XML. `Masters` is skipped without reading its attributes.
pub fn read_package_xml(xml: &[u8]) -> Result<BidPackage, QbError> {
    let xml = xml.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(xml);
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut pkg = BidPackage::default();
    let mut path: Vec<String> = Vec::new();
    let (mut saw_root, mut saw_bid) = (false, false);

    loop {
        let (e, is_start) = match reader
            .read_event_into(&mut buf)
            .map_err(|x| QbError::Xml(x.to_string()))?
        {
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(_) => {
                path.pop();
                buf.clear();
                continue;
            }
            Event::Eof => break,
            _ => {
                buf.clear();
                continue;
            }
        };
        let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        if path.is_empty() {
            if name != "BidPackage" {
                return Err(QbError::NotBidPackage);
            }
            saw_root = true;
            pkg.version = attrs_of(&e)?.s("Version");
        } else if path.len() == 1 && name == "Masters" {
            pkg.masters_skipped = true;
            if is_start {
                let end = e.to_end().into_owned();
                let mut skip = Vec::new();
                reader
                    .read_to_end_into(end.name(), &mut skip)
                    .map_err(|x| QbError::Xml(x.to_string()))?;
            }
            buf.clear();
            continue;
        } else if path.len() == 1 && name == "DBSettings" {
            pkg.db_bid_name = attrs_of(&e)?.s("BidName");
        } else if path.len() == 1 && name == "Bid" {
            saw_bid = true;
            pkg.bid = Bid::from_attrs(attrs_of(&e)?);
        } else if path.len() == 3 && path[1] == "Bid" && name == "Item" {
            route(&mut pkg, &path[2], attrs_of(&e)?);
        }
        if is_start {
            path.push(name);
        }
        buf.clear();
    }
    if !saw_root {
        return Err(QbError::NotBidPackage);
    }
    if !saw_bid {
        return Err(QbError::NoBid);
    }
    Ok(pkg)
}

fn route(pkg: &mut BidPackage, table: &str, a: Attrs) {
    match table {
        "BidConditions" => pkg.conditions.push(Condition::from_attrs(a)),
        "BidAreaConditions" => pkg.area_conditions.push(AreaCondition::from_attrs(a)),
        "BidItems" => pkg.items.push(BidItem::from_attrs(a)),
        "BidItemSizes" => pkg.item_sizes.push(ItemSize::from_attrs(a)),
        "BidDetails" => pkg.details.push(Detail::from_attrs(a)),
        "BidMarkups" => pkg.markups.push(Markup::from_attrs(a)),
        "BidWages" => pkg.wages.push(Wage::from_attrs(a)),
        "BidEquipment" => pkg.equipment.push(Charge::from_attrs(a)),
        "BidOthers" => pkg.others.push(Charge::from_attrs(a)),
        other => pkg
            .other_tables
            .entry(other.to_string())
            .or_default()
            .push(a.rest()),
    }
}
