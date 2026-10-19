//! Project file storage: one SQLite `.reaproj` file per project.
//!
//! Plans are linked by path and never copied in (OST-03-009). Enums, results and geometry
//! are stored as JSON text. Every `save_*` call replaces that entity set inside one
//! transaction: rows missing from the input are deleted, the rest are upserted.

use std::path::Path;

use rea_core::model::{
    BidArea, Condition, Geometry, Id, Measure, Page, Point, Project, Scale, Takeoff,
};
use rusqlite::{params, Connection, OpenFlags, Transaction};
use serde::{de::DeserializeOwned, Serialize};

/// Schema version this build writes.
pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("stored data is not valid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("file already exists: {0}")]
    Exists(String),
    #[error("project file has no project row")]
    NoProject,
    #[error("project file is from a newer version (schema {0})")]
    TooNew(i64),
}

pub type Result<T> = std::result::Result<T, StoreError>;

fn invalid<T>(msg: impl Into<String>) -> Result<T> {
    Err(StoreError::Invalid(msg.into()))
}

fn need_text(label: &str, s: &str) -> Result<()> {
    if s.trim().is_empty() {
        return invalid(format!("{label} must not be empty"));
    }
    Ok(())
}

fn need_finite(label: &str, v: f64) -> Result<()> {
    if !v.is_finite() {
        return invalid(format!("{label} must be a finite number"));
    }
    Ok(())
}

fn need_points(label: &str, pts: &[Point]) -> Result<()> {
    if pts.is_empty() {
        return invalid(format!("{label} has no points"));
    }
    pts.iter()
        .try_for_each(|p| need_finite(label, p.x).and_then(|_| need_finite(label, p.y)))
}

fn json<T: Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}

fn unjson<T: DeserializeOwned>(s: &str) -> Result<T> {
    Ok(serde_json::from_str(s)?)
}

/// Brings the file to `SCHEMA_VERSION`. Safe on an empty file and on an up-to-date one.
pub fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let current: Option<i64> =
        tx.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))?;
    let current = current.unwrap_or(0);
    if current > SCHEMA_VERSION {
        return Err(StoreError::TooNew(current));
    }
    if current < 1 {
        tx.execute_batch(
            "CREATE TABLE project (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, bid_no TEXT NOT NULL,
                job_no TEXT NOT NULL, estimator TEXT NOT NULL, notes TEXT NOT NULL,
                measure TEXT NOT NULL, created TEXT NOT NULL, origin_id TEXT);
             CREATE TABLE page (
                id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL REFERENCES project(id),
                idx INTEGER NOT NULL, sheet_no TEXT NOT NULL, sheet_name TEXT NOT NULL,
                file_path TEXT NOT NULL, file_page_index INTEGER NOT NULL,
                width REAL NOT NULL, height REAL NOT NULL, scale TEXT,
                rotation INTEGER NOT NULL, complete INTEGER NOT NULL);
             CREATE TABLE condition (
                id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL REFERENCES project(id),
                number INTEGER NOT NULL, name TEXT NOT NULL, style TEXT NOT NULL,
                ctype TEXT NOT NULL, layer TEXT NOT NULL, color TEXT NOT NULL,
                pattern TEXT NOT NULL, height REAL NOT NULL, thickness REAL NOT NULL,
                width REAL NOT NULL, depth REAL NOT NULL, slope TEXT, count_shape TEXT,
                results TEXT NOT NULL, UNIQUE (project_id, number));
             CREATE TABLE bid_area (
                id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL REFERENCES project(id),
                name TEXT NOT NULL, parent_id INTEGER REFERENCES bid_area(id),
                ord INTEGER NOT NULL);
             CREATE TABLE takeoff (
                id INTEGER PRIMARY KEY,
                page_id INTEGER NOT NULL REFERENCES page(id) ON DELETE CASCADE,
                condition_id INTEGER NOT NULL REFERENCES condition(id) ON DELETE CASCADE,
                bid_area_id INTEGER REFERENCES bid_area(id) ON DELETE SET NULL,
                negative INTEGER NOT NULL, geometry TEXT NOT NULL);
             CREATE INDEX takeoff_page ON takeoff(page_id);
             CREATE INDEX takeoff_condition ON takeoff(condition_id);
             INSERT INTO schema_version (version) VALUES (1);",
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Creates a new project file. Never overwrites an existing file.
    pub fn create(path: &Path, project: &Project) -> Result<Store> {
        need_text("project name", &project.name)?;
        if path.exists() {
            return Err(StoreError::Exists(path.display().to_string()));
        }
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn)?;
        let mut store = Store { conn };
        store.save_project(project)?;
        Ok(store)
    }

    /// Opens an existing project file and migrates it if it is older.
    pub fn open(path: &Path) -> Result<Store> {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn)?;
        Ok(Store { conn })
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))?)
    }

    pub fn save_project(&mut self, p: &Project) -> Result<()> {
        need_text("project name", &p.name)?;
        self.conn.execute(
            "INSERT INTO project (id, name, bid_no, job_no, estimator, notes, measure, created, origin_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET name=?2, bid_no=?3, job_no=?4, estimator=?5,
               notes=?6, measure=?7, created=?8, origin_id=?9",
            params![
                p.id,
                p.name,
                p.bid_no,
                p.job_no,
                p.estimator,
                p.notes,
                json(&p.measure)?,
                p.created,
                p.origin_id
            ],
        )?;
        Ok(())
    }

    pub fn load_project(&self) -> Result<Project> {
        let row = self.conn.query_row(
            "SELECT id, name, bid_no, job_no, estimator, notes, measure, created, origin_id
             FROM project ORDER BY id LIMIT 1",
            [],
            |r| {
                Ok((
                    Project {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        bid_no: r.get(2)?,
                        job_no: r.get(3)?,
                        estimator: r.get(4)?,
                        notes: r.get(5)?,
                        measure: Measure::Imperial,
                        created: r.get(7)?,
                        origin_id: r.get(8)?,
                    },
                    r.get::<_, String>(6)?,
                ))
            },
        );
        match row {
            Ok((mut p, measure)) => {
                p.measure = unjson(&measure)?;
                Ok(p)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(StoreError::NoProject),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_pages(&mut self, pages: &[Page]) -> Result<()> {
        for p in pages {
            need_text("page file path", &p.file_path)?;
            need_finite("page width", p.width)?;
            need_finite("page height", p.height)?;
            if p.width <= 0.0 || p.height <= 0.0 {
                return invalid("page size must be positive");
            }
            if let Some(s) = &p.scale {
                need_finite("scale", s.real_per_page_unit)?;
                if s.real_per_page_unit <= 0.0 {
                    return invalid("scale must be positive");
                }
            }
        }
        let tx = self.conn.transaction()?;
        sync(&tx, "page", pages.iter().map(|p| p.id))?;
        for p in pages {
            let scale = p.scale.as_ref().map(json).transpose()?;
            tx.execute(
                "INSERT INTO page (id, project_id, idx, sheet_no, sheet_name, file_path,
                   file_page_index, width, height, scale, rotation, complete)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET project_id=?2, idx=?3, sheet_no=?4,
                   sheet_name=?5, file_path=?6, file_page_index=?7, width=?8, height=?9,
                   scale=?10, rotation=?11, complete=?12",
                params![
                    p.id,
                    p.project_id,
                    p.index,
                    p.sheet_no,
                    p.sheet_name,
                    p.file_path,
                    p.file_page_index,
                    p.width,
                    p.height,
                    scale,
                    p.rotation,
                    p.complete
                ],
            )?;
        }
        Ok(tx.commit()?)
    }

    pub fn load_pages(&self) -> Result<Vec<Page>> {
        let mut st = self.conn.prepare(
            "SELECT id, project_id, idx, sheet_no, sheet_name, file_path, file_page_index,
               width, height, scale, rotation, complete FROM page ORDER BY idx, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                Page {
                    id: r.get(0)?,
                    project_id: r.get(1)?,
                    index: r.get(2)?,
                    sheet_no: r.get(3)?,
                    sheet_name: r.get(4)?,
                    file_path: r.get(5)?,
                    file_page_index: r.get(6)?,
                    width: r.get(7)?,
                    height: r.get(8)?,
                    scale: None,
                    rotation: r.get(10)?,
                    complete: r.get(11)?,
                },
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (mut p, scale) = row?;
            p.scale = scale.map(|s| unjson::<Scale>(&s)).transpose()?;
            out.push(p);
        }
        Ok(out)
    }

    pub fn save_conditions(&mut self, conditions: &[Condition]) -> Result<()> {
        for c in conditions {
            need_text("condition name", &c.name)?;
            for (label, v) in [
                ("height", c.height),
                ("thickness", c.thickness),
                ("width", c.width),
                ("depth", c.depth),
            ] {
                need_finite(label, v)?;
            }
            if let Some((rise, run)) = c.slope {
                need_finite("slope rise", rise)?;
                need_finite("slope run", run)?;
                if run == 0.0 {
                    return invalid("slope run must not be zero");
                }
            }
        }
        let tx = self.conn.transaction()?;
        sync(&tx, "condition", conditions.iter().map(|c| c.id))?;
        for c in conditions {
            tx.execute(
                "INSERT INTO condition (id, project_id, number, name, style, ctype, layer,
                   color, pattern, height, thickness, width, depth, slope, count_shape, results)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(id) DO UPDATE SET project_id=?2, number=?3, name=?4, style=?5,
                   ctype=?6, layer=?7, color=?8, pattern=?9, height=?10, thickness=?11,
                   width=?12, depth=?13, slope=?14, count_shape=?15, results=?16",
                params![
                    c.id,
                    c.project_id,
                    c.number,
                    c.name,
                    json(&c.style)?,
                    c.ctype,
                    c.layer,
                    json(&c.color)?,
                    c.pattern,
                    c.height,
                    c.thickness,
                    c.width,
                    c.depth,
                    c.slope.as_ref().map(json).transpose()?,
                    c.count_shape.as_ref().map(json).transpose()?,
                    json(&c.results)?
                ],
            )?;
        }
        Ok(tx.commit()?)
    }

    pub fn load_conditions(&self) -> Result<Vec<Condition>> {
        let mut st = self.conn.prepare(
            "SELECT id, project_id, number, name, style, ctype, layer, color, pattern, height,
               thickness, width, depth, slope, count_shape, results FROM condition ORDER BY number",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                Condition {
                    id: r.get(0)?,
                    project_id: r.get(1)?,
                    number: r.get(2)?,
                    name: r.get(3)?,
                    style: rea_core::model::ConditionStyle::Linear,
                    ctype: r.get(5)?,
                    layer: r.get(6)?,
                    color: [0; 3],
                    pattern: r.get(8)?,
                    height: r.get(9)?,
                    thickness: r.get(10)?,
                    width: r.get(11)?,
                    depth: r.get(12)?,
                    slope: None,
                    count_shape: None,
                    results: [None; 3],
                },
                // style, color, slope, count_shape, results as stored JSON text
                [
                    r.get::<_, Option<String>>(4)?,
                    r.get(7)?,
                    r.get(13)?,
                    r.get(14)?,
                    r.get(15)?,
                ],
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (mut c, [style, color, slope, shape, results]) = row?;
            c.style = unjson(&style.unwrap_or_default())?;
            c.color = unjson(&color.unwrap_or_default())?;
            c.slope = slope.map(|s| unjson(&s)).transpose()?;
            c.count_shape = shape.map(|s| unjson(&s)).transpose()?;
            c.results = unjson(&results.unwrap_or_default())?;
            out.push(c);
        }
        Ok(out)
    }

    pub fn save_bid_areas(&mut self, areas: &[BidArea]) -> Result<()> {
        for a in areas {
            need_text("bid area name", &a.name)?;
        }
        let tx = self.conn.transaction()?;
        // Defer the parent check so a whole tree can be written in any order.
        tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        sync(&tx, "bid_area", areas.iter().map(|a| a.id))?;
        for a in areas {
            tx.execute(
                "INSERT INTO bid_area (id, project_id, name, parent_id, ord)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET project_id=?2, name=?3, parent_id=?4, ord=?5",
                params![a.id, a.project_id, a.name, a.parent_id, a.order],
            )?;
        }
        Ok(tx.commit()?)
    }

    pub fn load_bid_areas(&self) -> Result<Vec<BidArea>> {
        let mut st = self.conn.prepare(
            "SELECT id, project_id, name, parent_id, ord FROM bid_area ORDER BY ord, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(BidArea {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                parent_id: r.get(3)?,
                order: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn save_takeoffs(&mut self, takeoffs: &[Takeoff]) -> Result<()> {
        for t in takeoffs {
            match &t.geometry {
                Geometry::Polyline(pts) => need_points("polyline", pts)?,
                Geometry::Polygon { outer, backouts } => {
                    need_points("polygon", outer)?;
                    backouts
                        .iter()
                        .try_for_each(|b| need_points("backout", b))?;
                }
                Geometry::Point(p) => {
                    need_finite("point x", p.x)?;
                    need_finite("point y", p.y)?;
                }
            }
        }
        let tx = self.conn.transaction()?;
        sync(&tx, "takeoff", takeoffs.iter().map(|t| t.id))?;
        for t in takeoffs {
            tx.execute(
                "INSERT INTO takeoff (id, page_id, condition_id, bid_area_id, negative, geometry)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET page_id=?2, condition_id=?3, bid_area_id=?4,
                   negative=?5, geometry=?6",
                params![
                    t.id,
                    t.page_id,
                    t.condition_id,
                    t.bid_area_id,
                    t.negative,
                    json(&t.geometry)?
                ],
            )?;
        }
        Ok(tx.commit()?)
    }

    pub fn load_takeoffs(&self) -> Result<Vec<Takeoff>> {
        let mut st = self.conn.prepare(
            "SELECT id, page_id, condition_id, bid_area_id, negative, geometry
             FROM takeoff ORDER BY id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                Takeoff {
                    id: r.get(0)?,
                    page_id: r.get(1)?,
                    condition_id: r.get(2)?,
                    bid_area_id: r.get(3)?,
                    negative: r.get(4)?,
                    geometry: Geometry::Point(Point { x: 0.0, y: 0.0 }),
                },
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (mut t, g) = row?;
            t.geometry = unjson(&g)?;
            out.push(t);
        }
        Ok(out)
    }
}

/// Deletes rows of `table` whose id is not in `keep`. `table` is always a literal above.
fn sync(tx: &Transaction, table: &str, keep: impl Iterator<Item = Id>) -> Result<()> {
    let keep: Vec<Id> = keep.collect();
    let existing: Vec<Id> = {
        let mut st = tx.prepare(&format!("SELECT id FROM {table}"))?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let mut del = tx.prepare(&format!("DELETE FROM {table} WHERE id = ?1"))?;
    for id in existing.into_iter().filter(|id| !keep.contains(id)) {
        del.execute([id])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_core::model::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("rea_store_{name}.reaproj"));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn project() -> Project {
        Project {
            id: 1,
            name: "Test job".into(),
            bid_no: "B-1".into(),
            job_no: "J-1".into(),
            estimator: "JC".into(),
            notes: "n".into(),
            measure: Measure::Imperial,
            created: "2026-10-20T00:00:00Z".into(),
            origin_id: Some("src".into()),
        }
    }

    fn page() -> Page {
        Page {
            id: 10,
            project_id: 1,
            index: 0,
            sheet_no: "A101".into(),
            sheet_name: "Plan".into(),
            file_path: "C:\\plans\\a.pdf".into(),
            file_page_index: 3,
            width: 2592.0,
            height: 1728.0,
            scale: Some(Scale {
                real_per_page_unit: 0.1 + 0.2,
                style: ScaleStyle::Architectural,
                label: "1/8\" = 1'-0\"".into(),
            }),
            rotation: 90,
            complete: true,
        }
    }

    fn condition() -> Condition {
        Condition {
            id: 20,
            project_id: 1,
            number: 1,
            name: "Wall".into(),
            style: ConditionStyle::Linear,
            ctype: "Framing".into(),
            layer: "Default".into(),
            color: [255, 0, 10],
            pattern: "solid".into(),
            height: 120.0,
            thickness: 3.625,
            width: 0.0,
            depth: 0.0,
            slope: Some((4.0, 12.0)),
            count_shape: Some(CountShape::Circle),
            results: [
                Some(ResultKind::Length),
                Some(ResultKind::SurfaceBothSides),
                None,
            ],
        }
    }

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn round_trip_every_entity() {
        let path = temp("round_trip");
        let areas = vec![
            BidArea {
                id: 31,
                project_id: 1,
                name: "Child".into(),
                parent_id: Some(30),
                order: 1,
            },
            BidArea {
                id: 30,
                project_id: 1,
                name: "Floor 1".into(),
                parent_id: None,
                order: 0,
            },
        ];
        let takeoffs = vec![
            Takeoff {
                id: 40,
                page_id: 10,
                condition_id: 20,
                bid_area_id: Some(30),
                negative: false,
                geometry: Geometry::Polyline(vec![pt(0.0, 0.0), pt(100.5, 20.25)]),
            },
            Takeoff {
                id: 41,
                page_id: 10,
                condition_id: 20,
                bid_area_id: None,
                negative: true,
                geometry: Geometry::Polygon {
                    outer: vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)],
                    backouts: vec![vec![pt(1.0, 1.0), pt(2.0, 1.0), pt(2.0, 2.0)]],
                },
            },
            Takeoff {
                id: 42,
                page_id: 10,
                condition_id: 20,
                bid_area_id: None,
                negative: false,
                geometry: Geometry::Point(pt(5.0, 6.0)),
            },
        ];
        {
            let mut s = Store::create(&path, &project()).unwrap();
            s.save_pages(&[page()]).unwrap();
            s.save_conditions(&[condition()]).unwrap();
            s.save_bid_areas(&areas).unwrap();
            s.save_takeoffs(&takeoffs).unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.load_project().unwrap(), project());
        assert_eq!(s.load_pages().unwrap(), vec![page()]);
        assert_eq!(s.load_conditions().unwrap(), vec![condition()]);
        let mut sorted = areas.clone();
        sorted.sort_by_key(|a| a.order);
        assert_eq!(s.load_bid_areas().unwrap(), sorted);
        assert_eq!(s.load_takeoffs().unwrap(), takeoffs);
        assert!(matches!(
            Store::create(&path, &project()),
            Err(StoreError::Exists(_))
        ));
    }

    #[test]
    fn migrates_empty_file() {
        let path = temp("migrate_empty");
        std::fs::write(&path, b"").unwrap();
        let mut conn = Connection::open(&path).unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        let v: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        drop(conn);
        assert_eq!(Store::open(&path).unwrap().schema_version().unwrap(), 1);
    }

    #[test]
    fn rejects_bad_input_and_does_not_half_write() {
        let path = temp("reject");
        let mut s = Store::create(&path, &project()).unwrap();
        s.save_pages(&[page()]).unwrap();
        let mut bad = page();
        bad.id = 11;
        bad.width = f64::NAN;
        assert!(matches!(
            s.save_pages(&[page(), bad]),
            Err(StoreError::Invalid(_))
        ));
        let mut c = condition();
        c.height = f64::INFINITY;
        assert!(matches!(
            s.save_conditions(&[c]),
            Err(StoreError::Invalid(_))
        ));
        let mut c = condition();
        c.name = "  ".into();
        assert!(matches!(
            s.save_conditions(&[c]),
            Err(StoreError::Invalid(_))
        ));
        let t = Takeoff {
            id: 1,
            page_id: 10,
            condition_id: 20,
            bid_area_id: None,
            negative: false,
            geometry: Geometry::Point(pt(f64::NAN, 0.0)),
        };
        assert!(matches!(s.save_takeoffs(&[t]), Err(StoreError::Invalid(_))));
        assert_eq!(s.load_pages().unwrap(), vec![page()]);
        let mut p = project();
        p.name.clear();
        assert!(Store::create(&temp("reject_name"), &p).is_err());
    }
}
