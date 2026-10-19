use rea_quickbid::{read_package, read_package_xml, summarize, QbError};
use std::io::Write;

/// Invented values only.
const XML: &str = r#"<BidPackage Version="1">
<DBSettings DBRequiresAuth="0" BidName="TEST BID"/>
<Bid UID="1" BidNo="42" JobName="Demo &amp; Co" BidTotal="1234.5" SiteContact="Secret Person" JobSitePhone="555" HoursPerDay="8">
<BidConditions TableId="17">
<Item UID="10" BidUID="1" Name="Wall A" MatTotal="100.5" LabTotal="50.25" UOM1="2" Height="33528" Notes="line1&#xd;&#xa;line2" Odd="x"/>
<Item UID="11" BidUID="1" Name="Wall B" MatTotal="" LabTotal="7"/>
</BidConditions>
<BidItems TableId="16"><Item UID="20" Code="STD" Description="Stud" MatPrice="2.5" Waste="5"/></BidItems>
<BidItemSizes TableId="9"><Item UID="30" BidItemUID="20" Name="3-5/8" Length="30480"/></BidItemSizes>
<BidDetails TableId="5">
<Item UID="40" BidConditionUID="10" BidItemUID="20" BidItemSizeUID="30" Quantity="3"/>
<Item UID="41" BidConditionUID="10" BidItemUID="20" BidItemSizeUID="" Quantity="1"/>
</BidDetails>
<BidWages TableId="3"><Item UID="50" BaseWage="20" Burden1="1.5" Burden8="2"/></BidWages>
<BidItemChains TableId="4"><Item UID="60" BidItemUID="20"/></BidItemChains>
</Bid>
<Masters>
<Contacts TableId="1"><Item UID="1" FullName="Nobody Real" Email="x@example.test"/></Contacts>
<MItem UID="2" Name="Also skipped"/>
</Masters>
</BidPackage>"#;

fn check(pkg: &rea_quickbid::BidPackage) {
    assert_eq!(pkg.version, "1");
    assert_eq!(pkg.db_bid_name, "TEST BID");
    assert_eq!(pkg.bid.bid_no, Some(42));
    assert_eq!(pkg.bid.job_name, "Demo & Co");
    assert!(pkg.masters_skipped);
    assert_eq!(pkg.conditions.len(), 2);
    assert_eq!(pkg.conditions[0].notes, "line1\r\nline2");
    assert_eq!(
        pkg.conditions[0].extras.get("Odd").map(String::as_str),
        Some("x")
    );
    assert_eq!(pkg.conditions[1].mat_total, None);
    assert_eq!(pkg.items[0].mat_price, Some(2.5));
    assert_eq!(pkg.wages[0].burden[0], Some(1.5));
    assert_eq!(pkg.wages[0].burden[7], Some(2.0));
    assert_eq!(pkg.other_tables["BidItemChains"].len(), 1);
    assert_eq!(pkg.details_for("10").count(), 2);
    assert_eq!(pkg.size("30").unwrap().name, "3-5/8");
    assert_eq!(pkg.item("20").unwrap().code, "STD");

    let s = summarize(pkg);
    assert_eq!(s.bid_total, Some(1234.5));
    assert_eq!(s.condition_count, 2);
    assert_eq!(s.item_count, 1);
    assert_eq!(s.mat_total_sum, 100.5);
    assert_eq!(s.lab_total_sum, 57.25);
    assert_eq!(s.conditions[0].detail_count, 2);

    // Personal data must not survive anywhere.
    let dump = format!("{pkg:?}");
    for bad in [
        "Secret Person",
        "Nobody Real",
        "x@example.test",
        "Also skipped",
        "555",
    ] {
        assert!(!dump.contains(bad), "leaked {bad}");
    }
}

#[test]
fn parses_xml_and_skips_masters() {
    check(&read_package_xml(XML.as_bytes()).unwrap());
}

#[test]
fn parses_xml_in_cab() {
    let mut builder = cab::CabinetBuilder::new();
    builder
        .add_folder(cab::CompressionType::None)
        .add_file("job.xml");
    let mut cur = std::io::Cursor::new(Vec::new());
    let mut w = builder.build(&mut cur).unwrap();
    while let Some(mut f) = w.next_file().unwrap() {
        f.write_all(XML.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    let out = cur.into_inner();
    assert!(out.starts_with(b"MSCF"));
    check(&read_package(&out).unwrap());
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    assert!(matches!(
        read_package_xml(b"<Other/>"),
        Err(QbError::NotBidPackage)
    ));
    assert!(matches!(read_package_xml(b""), Err(QbError::NotBidPackage)));
    assert!(matches!(
        read_package_xml(b"<BidPackage Version=\"1\"></BidPackage>"),
        Err(QbError::NoBid)
    ));
    assert!(read_package_xml(b"<BidPackage><Bid a=").is_err());
    assert!(read_package(b"MSCF garbage").is_err());
}

/// Run with `cargo test -p rea-quickbid -- --ignored --nocapture`. Prints counts and totals only.
#[test]
#[ignore]
fn real_sample_job() {
    let path = r"E:\opentakeoff-study\qb\sample-job.qbp";
    let Ok(bytes) = std::fs::read(path) else {
        eprintln!("sample not present, skipping");
        return;
    };
    let t = std::time::Instant::now();
    let pkg = read_package(&bytes).unwrap();
    let s = summarize(&pkg);
    println!(
        "parsed in {:?}, masters skipped: {}",
        t.elapsed(),
        pkg.masters_skipped
    );
    println!(
        "bid_no={:?} bid_total={:?} conditions={} items={} sizes={} details={}",
        s.bid_no, s.bid_total, s.condition_count, s.item_count, s.item_size_count, s.detail_count
    );
    println!(
        "markups={} wages={} equipment={} others={} area_conditions={}",
        pkg.markups.len(),
        pkg.wages.len(),
        pkg.equipment.len(),
        pkg.others.len(),
        pkg.area_conditions.len()
    );
    for (t, rows) in &pkg.other_tables {
        println!("table {t}: {} rows", rows.len());
    }
    println!("sum mat={:.4} lab={:.4}", s.mat_total_sum, s.lab_total_sum);
    assert!(s.condition_count > 0);
}
