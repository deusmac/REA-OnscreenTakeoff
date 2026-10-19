use std::io::Write;

use super::*;

/// Invented job: two pages, three conditions, one of each takeoff kind plus an arc.
const JOB: &str = r#"<XML_ROOT>
<OST Version="9.9.9"/>
<Bid UID="7" JobName="Test Hall" BidNo="12" JobID="J-1" Notes="a &amp; b" CreateDateTime="2030 1 2 3 4 5" MeasureBase="0" EstimatorUID="3" SourceBidUID="">
<BidLayers><BidLayer UID="20" Name="Walls"/></BidLayers>
<BidAreas><BidAreas/></BidAreas>
<BidConditions>
<BidCondition UID="100" RefNo="5" Name="Wall A" Type="0" Shape="-1" Pattern="8" ColorFill="16711680" ColorLine="0" Height="120" Thickness="4" Width="12" Depth="0" Rise="0" Run="0" BidLayerUID="20" CdnTypeUID="30" UOM1="2" Quantity1="1" Quantity2="3" Quantity3="0"/>
<BidCondition UID="101" RefNo="6" Name="Floor" Type="1" ColorFill="255" Quantity1="11" Quantity2="15" Quantity3="99"/>
<BidCondition UID="102" Name="Door" Type="2" Shape="2" Quantity1="23" Rise="1" Run="12"/>
</BidConditions>
<BidPages>
<BidPage UID="200" Name="PLAN" SheetNo="A1" ImagePath="D:\Jobs\x\set.pdf" Index1="2" Sequence="2" Width="36" Height="24" ScaleStyle="1" IsCustomScale="0" ScaleFactor1="0.125" ScaleFactor2="12" Rotation="0">
<BidTakeoffs>
<BidTakeoff UID="1" BidConditionUID="100" BidAreaUID="0" IsNegativeQuantity="0" Curve="-1" Position="100;200;100;800;0;0;0&#xa;"/>
<BidTakeoff UID="2" BidConditionUID="100" Curve="0" Position="0;0;0;100;50;50;-10"/>
<BidTakeoff UID="3" BidConditionUID="101" IsNegativeQuantity="1" Position="0;0;100;0;100;100;0;100"/>
<BidTakeoff UID="4" BidConditionUID="102" Position="250;350"/>
</BidTakeoffs>
</BidPage>
<BidPage UID="201" Name="NOSCALE" ImagePath="D:\Jobs\x\set.pdf" Index1="1" Width="17" Height="11" Rotation="90"><BidTakeoffs/></BidPage>
</BidPages>
</Bid>
<CdnTypes><CdnType UID="30" Name="WALLS"/></CdnTypes>
<Employees><Employee UID="1" LoginName="x" Password="SECRETPW"/></Employees>
</XML_ROOT>"#;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn maps_job() {
    let imp = import_bytes(JOB.as_bytes()).unwrap();
    assert_eq!(imp.ost_version, "9.9.9");
    assert!(imp.alternates.is_empty());
    let b = &imp.bid;
    assert_eq!(b.project.id, 7);
    assert_eq!(b.project.name, "Test Hall");
    assert_eq!(b.project.notes, "a & b");
    assert_eq!(b.project.created, "2030-01-02T03:04:05");
    assert_eq!(b.project.origin_id, None);
    assert_eq!(b.project_extra["EstimatorUID"], "3");

    let p = &b.pages[0];
    assert_eq!((p.id, p.index, p.file_page_index), (200, 0, 1));
    assert!(near(p.width, 36.0 * 72.0) && near(p.height, 24.0 * 72.0));
    let s = p.scale.as_ref().unwrap();
    assert_eq!(s.style, ScaleStyle::Architectural);
    // 1/8 in = 12 in on paper: 96 real inches per paper inch, 96/72 per point
    assert!(near(s.real_per_page_unit, 96.0 / 72.0));
    assert_eq!(b.page_extra[&200]["ScaleStyle"], "1");
    assert!(b.pages[1].scale.is_none());
    assert_eq!(b.pages[1].rotation, 90);

    let wall = &b.conditions[0];
    assert_eq!((wall.number, wall.style), (5, ConditionStyle::Linear));
    assert_eq!(
        (wall.layer.as_str(), wall.ctype.as_str()),
        ("Walls", "WALLS")
    );
    assert_eq!(wall.color, [0, 0, 255]);
    assert_eq!(wall.results, [Some(ResultKind::Length), None, None]);
    assert_eq!(b.condition_extra[&100]["Quantity2"], "3");
    assert_eq!(b.conditions[1].color, [255, 0, 0]);
    assert_eq!(
        b.conditions[1].results,
        [Some(ResultKind::Area), Some(ResultKind::Perimeter), None]
    );
    let door = &b.conditions[2];
    assert_eq!((door.number, door.slope), (102, Some((1.0, 12.0))));

    assert_eq!(b.takeoffs.len(), 4);
    // 100 hundredths of an inch = 1 in = 72 points
    match &b.takeoffs[0].geometry {
        Geometry::Polyline(v) => {
            assert_eq!(v.len(), 2);
            assert!(near(v[1].y, 8.0 * 72.0));
        }
        g => panic!("{g:?}"),
    }
    assert!(b.takeoff_raw[&1].arc.is_none());
    let arc = b.takeoff_raw[&2].arc.as_ref().unwrap();
    assert!(near(arc.sagitta, -0.1 * 72.0));
    assert!(near(arc.through.x, 0.5 * 72.0));
    assert!(b.takeoffs[2].negative);
    assert!(matches!(&b.takeoffs[2].geometry, Geometry::Polygon { outer, .. } if outer.len() == 4));
    assert!(matches!(b.takeoffs[3].geometry, Geometry::Point(pt) if near(pt.x, 2.5 * 72.0)));
    assert_eq!(b.takeoffs[3].bid_area_id, None);
}

#[test]
fn employees_never_read() {
    let imp = import_bytes(JOB.as_bytes()).unwrap();
    assert!(!format!("{imp:?}").contains("SECRETPW"));
}

#[test]
fn bad_input_is_an_error() {
    let no_cond = JOB.replace("BidConditionUID=\"102\"", "BidConditionUID=\"999\"");
    assert!(matches!(
        import_bytes(no_cond.as_bytes()),
        Err(OstError::UnknownRef { .. })
    ));
    let bad_num = JOB.replace("Width=\"36\"", "Width=\"wide\"");
    assert!(matches!(
        import_bytes(bad_num.as_bytes()),
        Err(OstError::BadValue { attr: "Width", .. })
    ));
    let nan = JOB.replace("Width=\"36\"", "Width=\"NaN\"");
    assert!(import_bytes(nan.as_bytes()).is_err());
    let no_uid = JOB.replace("UID=\"200\" ", "");
    assert!(matches!(
        import_bytes(no_uid.as_bytes()),
        Err(OstError::MissingUid { .. })
    ));
    let bad_pos = JOB.replace("250;350", "250;x");
    assert!(import_bytes(bad_pos.as_bytes()).is_err());
    let one_pt = JOB.replace("100;200;100;800;0;0;0&#xa;", "100;200");
    assert!(import_bytes(one_pt.as_bytes()).is_err());
    assert!(import_bytes(b"<XML_ROOT><OST/></XML_ROOT>").is_err());
    assert!(import_bytes(b"<XML_ROOT><Bid UID=\"1\">").is_err());
}

#[test]
fn archive_path_for_plans() {
    assert_eq!(
        plan_archive_path("E:\\Desktop\\Jobs\\set (2).pdf"),
        "TempImages!.tmp\\Desktop\\Jobs\\set (2).pdf"
    );
}

#[test]
fn reads_cabinet_package() {
    let mut b = cab::CabinetBuilder::new();
    let f = b.add_folder(cab::CompressionType::None);
    f.add_file("BidTrans.xml");
    f.add_file("job.ost");
    f.add_file("TempImages!.tmp\\Desktop\\x\\set.pdf");
    let mut out = std::io::Cursor::new(Vec::new());
    let mut w = b.build(&mut out).unwrap();
    while let Some(mut fw) = w.next_file().unwrap() {
        let name = fw.file_name().to_string();
        match name.as_str() {
            "job.ost" => fw.write_all(JOB.as_bytes()).unwrap(),
            "BidTrans.xml" => fw.write_all(b"<XML_ROOT/>").unwrap(),
            _ => fw.write_all(b"%PDF-fake").unwrap(),
        }
    }
    w.finish().unwrap();

    let imp = import_bytes(out.get_ref()).unwrap();
    assert_eq!(imp.bid.takeoffs.len(), 4);
    assert_eq!(imp.plan_files.len(), 1);
    assert_eq!(
        imp.plan_files[0].archive_path,
        "TempImages!.tmp\\Desktop\\x\\set.pdf"
    );
    assert_eq!(imp.plan_files[0].bytes, b"%PDF-fake");
}

#[test]
fn package_without_job_file_fails() {
    let mut b = cab::CabinetBuilder::new();
    b.add_folder(cab::CompressionType::None)
        .add_file("BidTrans.xml");
    let mut out = std::io::Cursor::new(Vec::new());
    let mut w = b.build(&mut out).unwrap();
    while let Some(mut fw) = w.next_file().unwrap() {
        fw.write_all(b"<a/>").unwrap();
    }
    w.finish().unwrap();
    assert!(matches!(
        import_bytes(out.get_ref()),
        Err(OstError::NoJobFile)
    ));
}

/// Real job on JC's machine; prints counts only (no client data).
#[test]
#[ignore]
fn real_sample_job() {
    let path = std::path::Path::new(r"E:\opentakeoff-study\ost\sample-job.osp");
    if !path.exists() {
        eprintln!("sample not present, skipping");
        return;
    }
    let imp = import_path(path).unwrap();
    let b = &imp.bid;
    println!(
        "version {} pages {} conditions {} takeoffs {} areas {} alternates {} plan files {}",
        imp.ost_version,
        b.pages.len(),
        b.conditions.len(),
        b.takeoffs.len(),
        b.bid_areas.len(),
        imp.alternates.len(),
        imp.plan_files.len()
    );
    for a in &imp.alternates {
        println!(
            "alt pages {} conditions {} takeoffs {}",
            a.pages.len(),
            a.conditions.len(),
            a.takeoffs.len()
        );
    }
    let (mut mx, mut my) = (0f64, 0f64);
    for t in &b.takeoffs {
        let pts: Vec<Point> = match &t.geometry {
            Geometry::Polyline(v) => v.clone(),
            Geometry::Polygon { outer, .. } => outer.clone(),
            Geometry::Point(p) => vec![*p],
        };
        for p in pts {
            mx = mx.max(p.x);
            my = my.max(p.y);
        }
    }
    println!("max takeoff x/y in points: {mx:.1} / {my:.1}");
}
