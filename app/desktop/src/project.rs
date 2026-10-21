//! Project files: new from a plan PDF, open a `.reaproj` or an OST job, save.
//!
//! Plans are linked by path and never copied into the project file (OST-03-009). An OST
//! job's plans are unpacked to a temp folder while it is open and moved to a
//! `<project>_plans` folder next to the `.reaproj` on the first save.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rea_core::model::{Measure, Page, Project};
use rea_pdf::PlanFile;
use rea_store::Store;

use crate::doc::{result_from_code, Doc, OstSource};

pub const EXT: &str = "reaproj";

fn stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(|| "Project".into(), |s| s.to_string_lossy().into_owned())
}

/// A new, unsaved project with every page of the PDF, no scales yet.
pub fn new_from_pdf(pdf: &Path) -> Result<Doc, String> {
    let file = PlanFile::open(pdf).map_err(|e| e.to_string())?;
    let path = std::path::absolute(pdf).unwrap_or_else(|_| pdf.to_path_buf());
    let pages = (0..file.page_count())
        .map(|i| {
            let (w, h) = file.page_size(i).unwrap_or((1.0, 1.0));
            Page {
                id: i as i64 + 2,
                project_id: 1,
                index: i as u32,
                sheet_no: String::new(),
                sheet_name: format!("Page {}", i + 1),
                file_path: path.to_string_lossy().into_owned(),
                file_page_index: i as u32,
                width: w as f64,
                height: h as f64,
                scale: None,
                rotation: 0,
                complete: false,
            }
        })
        .collect::<Vec<_>>();
    if pages.is_empty() {
        return Err(format!("{} has no pages", pdf.display()));
    }
    Ok(Doc::new(
        Project {
            id: 1,
            name: stem(pdf),
            bid_no: String::new(),
            job_no: String::new(),
            estimator: String::new(),
            notes: String::new(),
            measure: Measure::Imperial,
            created: String::new(),
            origin_id: None,
        },
        pages,
    ))
}

/// Name of the `.reaproj` extra holding an imported OST job's own fields (arcs, grids, zones,
/// typical areas, parent links, result codes), so a saved import keeps OST's numbers.
const OST_EXTRA: &str = "ost_source";

pub fn open_reaproj(path: &Path) -> Result<Doc, String> {
    let s = Store::open(path).map_err(|e| e.to_string())?;
    let err = |e: rea_store::StoreError| e.to_string();
    let mut doc = Doc::new(s.load_project().map_err(err)?, s.load_pages().map_err(err)?);
    doc.conditions = s.load_conditions().map_err(err)?;
    doc.takeoffs = s.load_takeoffs().map_err(err)?;
    doc.bid_areas = s.load_bid_areas().map_err(err)?;
    if let Some(ost) = s.load_extra::<OstSource>(OST_EXTRA).map_err(err)? {
        doc.ost = ost;
    }
    Ok(doc)
}

/// Opens an OST job. Returns the project and the temp folder its plan PDFs were unpacked
/// to (Pdfium reads from files).
pub fn open_ost(path: &Path) -> Result<(Doc, PathBuf), String> {
    let imp = rea_ost::import_path(path).map_err(|e| e.to_string())?;
    let dir = std::env::temp_dir().join("opentakeoff").join(stem(path));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut unpacked: HashMap<String, String> = HashMap::new();
    for (i, pf) in imp.plan_files.iter().enumerate() {
        if !pf.archive_path.to_ascii_lowercase().ends_with(".pdf") {
            continue; // shortcut: image plan pages (TIFF) show blank until we render images
        }
        let out = dir.join(format!("plan{i}.pdf"));
        std::fs::write(&out, &pf.bytes).map_err(|e| e.to_string())?;
        unpacked.insert(
            pf.archive_path.to_ascii_lowercase(),
            out.to_string_lossy().into_owned(),
        );
    }

    let b = imp.bid;
    let job = rea_ost::totals_job(&b);
    let mut pages = b.pages;
    pages.sort_by_key(|p| p.index);
    for p in &mut pages {
        let key = rea_ost::plan_archive_path(&p.file_path).to_ascii_lowercase();
        if let Some(f) = unpacked.get(&key) {
            p.file_path = f.clone();
        } else if p.file_path.trim().is_empty() {
            p.file_path = "(no plan file)".into();
        }
    }
    let mut project = b.project;
    if project.name.trim().is_empty() {
        project.name = stem(path);
    }
    let mut doc = Doc::new(project, pages);

    // Conditions take every result code we can name, so a save keeps them; the store needs
    // a name, a unique number and a non-zero slope run.
    let mut numbers = HashSet::new();
    let top = b.conditions.iter().map(|c| c.number).max().unwrap_or(0);
    let mut spare = top + 1;
    let ost_by_id: HashMap<_, _> = job.conditions.iter().map(|o| (o.uid, o)).collect();
    for mut c in b.conditions {
        if !numbers.insert(c.number) {
            c.number = spare;
            spare += 1;
        }
        if c.name.trim().is_empty() {
            c.name = format!("Condition {}", c.number);
        }
        if c.slope.is_some_and(|(_, run)| run == 0.0) {
            c.slope = None;
        }
        if let Some(o) = ost_by_id.get(&c.id) {
            c.results = std::array::from_fn(|i| {
                (o.uoms[i] != -1)
                    .then(|| result_from_code(c.style, o.results[i]))
                    .flatten()
            });
            doc.ost.conditions.insert(c.id, (c.results, (*o).clone()));
        }
        doc.conditions.push(c);
    }
    let geometry: HashMap<_, _> = b.takeoffs.iter().map(|t| (t.id, &t.geometry)).collect();
    doc.ost = OstSource {
        takeoffs: job
            .takeoffs
            .into_iter()
            .filter_map(|o| Some((o.uid, ((*geometry.get(&o.uid)?).clone(), o))))
            .collect(),
        zone_outlines: job.zone_outlines,
        negative_zones: job.negative_zones,
        typ_area_counts: job.typ_area_counts,
        conditions: std::mem::take(&mut doc.ost.conditions),
    };
    doc.takeoffs = b.takeoffs;
    doc.bid_areas = b.bid_areas;
    Ok((doc, dir))
}

/// Writes the project to `path`, replacing it only once the new file is complete. Plans
/// still in `unpacked` (an OST job's temp folder) move to `<stem>_plans` next to it first.
pub fn save(doc: &mut Doc, path: &Path, unpacked: Option<&Path>) -> Result<(), String> {
    if let Some(tmp) = unpacked {
        let dir = path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("{}_plans", stem(path)));
        let mut moved = HashMap::new();
        for p in &doc.pages {
            let from = Path::new(&p.file_path);
            if !local_plan_path(&p.file_path)
                || !from.starts_with(tmp)
                || moved.contains_key(&p.file_path)
            {
                continue;
            }
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let to = dir.join(from.file_name().unwrap_or_default());
            std::fs::copy(from, &to).map_err(|e| format!("Could not copy plans: {e}"))?;
            moved.insert(p.file_path.clone(), to.to_string_lossy().into_owned());
        }
        doc.relink(&moved);
    }

    let part = path.with_extension(format!("{EXT}.saving"));
    let _ = std::fs::remove_file(&part);
    let write = || -> rea_store::Result<()> {
        let mut s = Store::create(&part, &doc.project)?;
        s.save_pages(&doc.pages)?;
        s.save_conditions(&doc.conditions)?;
        s.save_bid_areas(&doc.bid_areas)?;
        s.save_takeoffs(&doc.takeoffs)?;
        if doc.ost != OstSource::default() {
            s.save_extra(OST_EXTRA, &doc.ost)?;
        }
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Could not save: {e}"));
    }
    std::fs::rename(&part, path).map_err(|e| format!("Could not save: {e}"))?;
    doc.saved_rev = doc.rev;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::tests::filled;

    #[test]
    fn save_and_reopen_round_trip() {
        let path = std::env::temp_dir().join("opentakeoff_round_trip.reaproj");
        let _ = std::fs::remove_file(&path);
        let mut d = filled();
        let before = d.summary();
        save(&mut d, &path, None).unwrap();
        assert!(!d.modified());
        // A second save replaces the file in place.
        save(&mut d, &path, None).unwrap();

        let r = open_reaproj(&path).unwrap();
        assert_eq!(r.project, d.project);
        assert_eq!(r.pages, d.pages);
        assert_eq!(r.conditions, d.conditions);
        assert_eq!(r.takeoffs, d.takeoffs);
        let after = r.summary();
        for (a, b) in before.iter().zip(&after) {
            assert_eq!(a.qty, b.qty);
        }
        assert_eq!(after.len(), 3);
        let _ = std::fs::remove_file(&path);
    }

    /// Real job from JC's corpus (not in the repo): run with `-- --ignored`.
    #[test]
    #[ignore]
    fn saved_ost_job_keeps_exact_totals() {
        let dir = Path::new(r"E:\opentakeoff-study\corpus\ost");
        let osp = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.to_string_lossy().contains("Sweat_440"))
            .unwrap();
        let (mut d, tmp) = open_ost(&osp).unwrap();
        let before = d.summary();
        let path = std::env::temp_dir().join("opentakeoff_ost_round_trip.reaproj");
        let _ = std::fs::remove_file(&path);
        save(&mut d, &path, Some(&tmp)).unwrap();
        let r = open_reaproj(&path).unwrap();
        assert_eq!(r.ost, d.ost);
        let after = r.summary();
        assert_eq!(before.len(), after.len());
        for (a, b) in before.iter().zip(&after) {
            assert_eq!(a.qty, b.qty);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_moves_unpacked_plans_next_to_project() {
        let root = std::env::temp_dir().join("opentakeoff_plans_test");
        let _ = std::fs::remove_dir_all(&root);
        let tmp = root.join("unpacked");
        std::fs::create_dir_all(&tmp).unwrap();
        let plan = tmp.join("plan0.pdf");
        std::fs::write(&plan, b"%PDF-1.4").unwrap();
        let mut d = filled();
        for p in &mut d.pages {
            p.file_path = plan.to_string_lossy().into_owned();
        }
        let path = root.join("job.reaproj");
        save(&mut d, &path, Some(&tmp)).unwrap();
        let moved = root.join("job_plans").join("plan0.pdf");
        assert!(moved.exists());
        let r = open_reaproj(&path).unwrap();
        assert!(r.pages.iter().all(|p| Path::new(&p.file_path) == moved));
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// Plan paths come from project and job files, which may come from someone else: only open
/// local absolute paths. A `\\server\share` (or `\\?\`) path would make Windows send the
/// user's login to that server, and `..` could step out of the expected folder.
pub(crate) fn local_plan_path(p: &str) -> bool {
    use std::path::{Component, Prefix};
    let path = Path::new(p);
    // Only a drive letter prefix (C:\); UNC, verbatim and device prefixes in any slash mix fail.
    matches!(path.components().next(), Some(Component::Prefix(pre)) if matches!(pre.kind(), Prefix::Disk(_)))
        && path.is_absolute()
        && !path.components().any(|c| matches!(c, Component::ParentDir))
}

#[cfg(test)]
mod plan_path_tests {
    use super::local_plan_path;

    #[test]
    fn rejects_unc_device_relative_and_parent_paths() {
        assert!(local_plan_path(r"E:\jobs\plans.pdf"));
        assert!(!local_plan_path(r"\\attacker\share\plans.pdf"));
        assert!(!local_plan_path(r"\\?\C:\plans.pdf"));
        assert!(!local_plan_path("//attacker/share/plans.pdf"));
        assert!(!local_plan_path(r"/\attacker\share\plans.pdf"));
        assert!(!local_plan_path(r"\/attacker/share/plans.pdf"));
        assert!(!local_plan_path(r"\\.\C:\plans.pdf"));
        assert!(!local_plan_path(r"plans.pdf"));
        assert!(!local_plan_path(r"C:\temp\opentakeoff\..\..\secret.pdf"));
    }
}
