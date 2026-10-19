//! Summary export to Excel (OST-12-xxx grid, M7).
//!
//! The Summary grid is written as a plain sheet: optional project and date lines, a header
//! row, then group header rows and Condition rows in grid order. Quantities are real
//! numbers; the column's display decimals only set the cell number format, so Excel keeps
//! full precision underneath.

use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Format, Workbook, XlsxError};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("could not build the workbook: {0}")]
    Xlsx(#[from] XlsxError),
    #[error("could not write the export file: {0}")]
    Io(#[from] std::io::Error),
    #[error("row {row} has {got} quantity cells but the table has {want} columns")]
    CellCount { row: usize, got: usize, want: usize },
}

/// One result column (Q1 to Q3 in the grid).
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub header: String,
    /// Unit label such as "LF" or "SF"; empty for none.
    pub unit: String,
    /// Display decimals, applied as the cell number format only.
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// Page, type, zone or area header line.
    Group(String),
    Condition {
        number: String,
        name: String,
        /// One entry per column; `None` leaves the cell empty.
        cells: Vec<Option<f64>>,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SummaryTable {
    /// unknown: the guide does not say whether the Excel export carries a title block
    /// (CSV has no header row, OST-16-011); we write one when these are set.
    pub project_name: Option<String>,
    /// Preformatted date text, so this crate needs no date dependency.
    pub exported_on: Option<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
}

/// Writes the table to `path`. The workbook is saved to a temp file in the same folder and
/// renamed over the target, so a failed export never leaves a broken file in place.
pub fn write_summary(path: &Path, table: &SummaryTable) -> Result<(), ExportError> {
    for (i, row) in table.rows.iter().enumerate() {
        if let Row::Condition { cells, .. } = row {
            if cells.len() != table.columns.len() {
                return Err(ExportError::CellCount {
                    row: i,
                    got: cells.len(),
                    want: table.columns.len(),
                });
            }
        }
    }
    let mut book = build(table)?;
    let tmp = temp_path(path);
    let result = book
        .save(&tmp)
        .map_err(ExportError::from)
        .and_then(|()| std::fs::rename(&tmp, path).map_err(ExportError::from));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

fn build(table: &SummaryTable) -> Result<Workbook, XlsxError> {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Summary")?;
    let bold = Format::new().set_bold();
    let mut r: u32 = 0;
    if let Some(p) = &table.project_name {
        sheet.write_string_with_format(r, 0, format!("Project: {p}"), &bold)?;
        r += 1;
    }
    if let Some(d) = &table.exported_on {
        sheet.write_string(r, 0, format!("Exported: {d}"))?;
        r += 1;
    }
    if r > 0 {
        r += 1; // blank spacer line
    }
    sheet.write_string_with_format(r, 0, "Condition No.", &bold)?;
    sheet.write_string_with_format(r, 1, "Condition", &bold)?;
    let mut formats = Vec::with_capacity(table.columns.len());
    for (i, c) in table.columns.iter().enumerate() {
        let head = if c.unit.is_empty() {
            c.header.clone()
        } else {
            format!("{} ({})", c.header, c.unit)
        };
        sheet.write_string_with_format(r, 2 + i as u16, head, &bold)?;
        formats.push(Format::new().set_num_format(num_format(c.decimals)));
    }
    sheet.set_freeze_panes(r + 1, 0)?;
    for row in &table.rows {
        r += 1;
        match row {
            Row::Group(label) => {
                sheet.write_string_with_format(r, 0, label, &bold)?;
            }
            Row::Condition {
                number,
                name,
                cells,
            } => {
                sheet.write_string(r, 0, number)?;
                sheet.write_string(r, 1, name)?;
                for (i, cell) in cells.iter().enumerate() {
                    if let Some(v) = cell {
                        sheet.write_number_with_format(r, 2 + i as u16, *v, &formats[i])?;
                    }
                }
            }
        }
    }
    sheet.autofit();
    Ok(book)
}

fn num_format(decimals: u8) -> String {
    if decimals == 0 {
        "0".to_string()
    } else {
        format!("0.{}", "0".repeat(decimals as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{open_workbook, Data, Reader, Xlsx};

    fn sample() -> SummaryTable {
        SummaryTable {
            project_name: Some("Test Job".into()),
            exported_on: Some("2026-10-20".into()),
            columns: vec![
                Column {
                    header: "Q1".into(),
                    unit: "LF".into(),
                    decimals: 2,
                },
                Column {
                    header: "Q2".into(),
                    unit: "SF".into(),
                    decimals: 0,
                },
            ],
            rows: vec![
                Row::Group("Page A1.01".into()),
                Row::Condition {
                    number: "1".into(),
                    name: "Wall 362S".into(),
                    cells: vec![Some(123.456789), Some(1000.5)],
                },
                Row::Condition {
                    number: "2".into(),
                    name: "Ceiling".into(),
                    cells: vec![None, Some(7.0)],
                },
            ],
        }
    }

    fn cells(path: &Path) -> Vec<Vec<Data>> {
        let mut wb: Xlsx<_> = open_workbook(path).unwrap();
        let range = wb.worksheet_range("Summary").unwrap();
        range.rows().map(|r| r.to_vec()).collect()
    }

    #[test]
    fn writes_headers_groups_and_full_precision_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.xlsx");
        write_summary(&path, &sample()).unwrap();
        let rows = cells(&path);
        assert_eq!(rows[0][0], Data::String("Project: Test Job".into()));
        assert_eq!(rows[1][0], Data::String("Exported: 2026-10-20".into()));
        assert_eq!(rows[3][2], Data::String("Q1 (LF)".into()));
        assert_eq!(rows[3][3], Data::String("Q2 (SF)".into()));
        assert_eq!(rows[4][0], Data::String("Page A1.01".into()));
        assert_eq!(rows[5][1], Data::String("Wall 362S".into()));
        assert_eq!(rows[5][2], Data::Float(123.456789));
        assert_eq!(rows[5][3], Data::Float(1000.5));
        assert_eq!(rows[6][2], Data::Empty);
        assert!(!dir.path().join("s.xlsx.tmp").exists());
    }

    #[test]
    fn overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.xlsx");
        std::fs::write(&path, b"old").unwrap();
        write_summary(&path, &sample()).unwrap();
        assert!(!cells(&path).is_empty());
    }

    #[test]
    fn bad_row_width_fails_and_keeps_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.xlsx");
        std::fs::write(&path, b"old").unwrap();
        let mut t = sample();
        t.rows.push(Row::Condition {
            number: "3".into(),
            name: "x".into(),
            cells: vec![Some(1.0)],
        });
        assert!(matches!(
            write_summary(&path, &t),
            Err(ExportError::CellCount { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
    }
}
