//! Plan PDF loading and page rendering, on top of the bundled Pdfium engine.
//!
//! Page coordinates are PDF points (1/72 in) with the origin at the top-left corner of the
//! page as displayed. A render scale is output pixels per point.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use pdfium_render::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PDF engine not found: {0}")]
    Engine(String),
    #[error("{0}")]
    Pdf(#[from] PdfiumError),
    #[error("page {0} does not exist")]
    NoPage(usize),
}

/// An RGBA image, row-major, 4 bytes per pixel.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// One opened plan PDF. Pages are rendered on demand.
///
/// Use Pdfium from one thread only: two threads opening or rendering at once crash it
/// (seen about 1 run in 30), even with the `thread_safe` feature.
pub struct PlanFile {
    doc: PdfDocument<'static>,
    sizes: Vec<(f32, f32)>,
}

impl PlanFile {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let doc = engine()?.load_pdf_from_file(path, None)?;
        let sizes = doc
            .pages()
            .iter()
            .map(|p| (p.width().value, p.height().value))
            .collect();
        Ok(Self { doc, sizes })
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    /// Page width and height in points, as displayed (page rotation applied).
    pub fn page_size(&self, index: usize) -> Option<(f32, f32)> {
        self.sizes.get(index).copied()
    }

    /// Renders the whole page at `scale` pixels per point.
    pub fn render_page(&self, index: usize, scale: f32) -> Result<Image, Error> {
        let (w, h) = self.page_size(index).ok_or(Error::NoPage(index))?;
        let (pw, ph) = ((w * scale).round() as i32, (h * scale).round() as i32);
        self.render_region(index, scale, 0, 0, pw.max(1), ph.max(1))
    }

    /// Renders only the `width` x `height` pixel window whose top-left corner sits at pixel
    /// (`x`, `y`) of the page rendered at `scale`. Memory stays proportional to the window, so
    /// a 36 x 48 sheet can be shown sharp at any zoom.
    pub fn render_region(
        &self,
        index: usize,
        scale: f32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Image, Error> {
        let (w, h) = self.page_size(index).ok_or(Error::NoPage(index))?;
        let page = self
            .doc
            .pages()
            .get(index as PdfPageIndex)
            .map_err(|_| Error::NoPage(index))?;
        let config = PdfRenderConfig::new()
            .set_fixed_size((w * scale).round() as i32, (h * scale).round() as i32)
            .render_form_data(false)
            .render_annotations(true)
            .clip(0, 0, width, height)
            .reset_matrix(PdfMatrix::new(scale, 0.0, 0.0, scale, -x as f32, -y as f32))?;
        let mut bitmap = PdfBitmap::empty(width, height, PdfBitmapFormat::BGRA)?;
        page.render_into_bitmap_with_config(&mut bitmap, &config)?;
        Ok(Image {
            width: width as usize,
            height: height as usize,
            rgba: bitmap.as_rgba_bytes(),
        })
    }
}

/// Folders searched for pdfium.dll: next to the program first, then the repo copy.
fn engine_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(dir);
    }
    dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium"));
    dirs
}

fn engine() -> Result<&'static Pdfium, Error> {
    static ENGINE: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            for dir in engine_dirs() {
                let lib = Pdfium::pdfium_platform_library_name_at_path(&dir);
                if lib.exists() {
                    return Pdfium::bind_to_library(&lib)
                        .map(Pdfium::new)
                        .map_err(|e| format!("{}: {e}", lib.display()));
                }
            }
            Err(format!("pdfium.dll missing; looked in {:?}", engine_dirs()))
        })
        .as_ref()
        .map_err(|e| Error::Engine(e.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Tests run in parallel threads; Pdfium must not (see `PlanFile`).
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// Writes a 2-page PDF whose pages are 200 x 100 pt with the bottom-left quarter filled black.
    /// Each test gets its own file: Pdfium reads lazily, so a shared file rewritten by a
    /// parallel test crashes the reader.
    fn fixture(name: &str) -> PathBuf {
        let content = "0 0 100 50 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 5 0 R >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 5 0 R >>".to_string(),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf += &format!("{} 0 obj\n{body}\nendobj\n", i + 1);
        }
        let xref = pdf.len();
        pdf += &format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
        for o in offsets {
            pdf += &format!("{o:010} 00000 n \n");
        }
        pdf += &format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        );
        let path = std::env::temp_dir().join(format!("rea-pdf-{name}-{}.pdf", std::process::id()));
        std::fs::write(&path, pdf).unwrap();
        path
    }

    fn is_black(img: &Image, x: usize, y: usize) -> bool {
        img.rgba[(y * img.width + x) * 4] < 64
    }

    #[test]
    fn opens_and_renders() {
        let _lock = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let plan = PlanFile::open(&fixture("open")).unwrap();
        assert_eq!(plan.page_count(), 2);
        assert_eq!(plan.page_size(1), Some((200.0, 100.0)));

        let full = plan.render_page(0, 2.0).unwrap();
        assert_eq!((full.width, full.height), (400, 200));
        assert!(is_black(&full, 10, 190) && !is_black(&full, 10, 10) && !is_black(&full, 390, 190));
    }

    #[test]
    fn region_matches_crop_of_full_render() {
        let _lock = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let plan = PlanFile::open(&fixture("region")).unwrap();
        let full = plan.render_page(0, 2.0).unwrap();
        let (x, y, w, h) = (150, 80, 100, 50);
        let part = plan.render_region(0, 2.0, x, y, w, h).unwrap();
        assert_eq!((part.width, part.height), (100, 50));
        for py in 0..h as usize {
            for px in 0..w as usize {
                let a = &part.rgba[(py * part.width + px) * 4..][..4];
                let b = &full.rgba[((py + y as usize) * full.width + px + x as usize) * 4..][..4];
                assert_eq!(a, b, "pixel {px},{py}");
            }
        }
    }
}
