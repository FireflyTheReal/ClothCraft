//! Physically scaled pattern sheets for printing. CPU only and deterministic:
//! the same template, paper and scale always produce the same bytes.
//!
//! One user unit is one millimetre, and each page is declared with `mm` units,
//! so a 100 mm boundary measures 100 mm of paper. Panels larger than the page
//! are tiled with a fixed overlap and corner registration marks; the sewing and
//! cutting contours are always drawn as two distinct lines.
use super::{GarmentTemplate, polygon::PolygonPattern, template::Panel};
use glam::DVec2;
use std::{
    fmt::Write as _,
    fs,
    path::{Path as FsPath, PathBuf},
};

/// ISO / US paper sizes in millimetres, portrait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Paper {
    #[default]
    A4,
    A3,
    Letter,
}

impl Paper {
    pub fn millimetres(self) -> (f64, f64) {
        match self {
            Paper::A4 => (210., 297.),
            Paper::A3 => (297., 420.),
            Paper::Letter => (215.9, 279.4),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A3 => "A3",
            Paper::Letter => "Letter",
        }
    }
}

/// Printed length of one material metre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrintScale {
    #[default]
    Full,
    Half,
    Quarter,
}

impl PrintScale {
    /// Millimetres of paper per material metre.
    pub fn millimetres_per_metre(self) -> f64 {
        match self {
            PrintScale::Full => 1000.,
            PrintScale::Half => 500.,
            PrintScale::Quarter => 250.,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PrintScale::Full => "1:1",
            PrintScale::Half => "1:2",
            PrintScale::Quarter => "1:4",
        }
    }
}

/// Blank border kept clear of geometry, in millimetres.
pub const MARGIN_MM: f64 = 10.;
/// How much consecutive tiles repeat, in millimetres, so they can be taped.
pub const OVERLAP_MM: f64 = 10.;
/// Hard ceiling on generated sheets, so a bad scale cannot fill a disk.
pub const MAX_SHEETS: usize = 400;
/// Calibration ruler length drawn on every sheet, in millimetres.
pub const RULER_MM: f64 = 100.;
/// Ruler tick spacing, in millimetres.
pub const RULER_TICK_MM: f64 = 10.;

/// Everything one panel contributes to a printed sheet, in millimetres with the
/// panel's printed origin at `(0, 0)`. Presentation-free and unit-testable.
#[derive(Clone, Debug)]
pub struct SheetGeometry {
    /// Cutting rings: the allowance outline, or the sewing rings when no
    /// allowance is stored. Counter-clockwise outers, clockwise holes.
    pub cutting: Vec<Vec<DVec2>>,
    /// Sewing rings, in the same winding convention.
    pub sewing: Vec<Vec<DVec2>>,
    /// Notch ticks as `[on_boundary, tip]`.
    pub notches: Vec<[DVec2; 2]>,
    /// Sampled internal lines.
    pub lines: Vec<Vec<DVec2>>,
    /// Grain guide `[start, end]`, empty when the line does not span the piece.
    pub grain: Vec<DVec2>,
    /// Printed offset of the sheet origin: the cutting contour's minimum, so the
    /// piece always starts at the margin even when an allowance extends outward.
    pub origin_mm: DVec2,
    /// Printed size of the cutting contour plus one margin on each side.
    pub extent_mm: DVec2,
    /// True when a seam allowance separates the cutting and sewing contours.
    pub has_allowance: bool,
}

/// Longest span of a contour set along an infinite line through `centre`.
fn span(contours: &[Vec<DVec2>], centre: DVec2, direction: DVec2) -> Option<[f64; 2]> {
    let mut hits = vec![];
    for contour in contours {
        for i in 0..contour.len() {
            let a = contour[i];
            let edge = contour[(i + 1) % contour.len()] - a;
            let det = direction.perp_dot(edge);
            if det.abs() < 1e-12 {
                continue;
            }
            let t = (a - centre).perp_dot(edge) / det;
            let u = (a - centre).perp_dot(direction) / det;
            if (-1e-10..=1.0 + 1e-10).contains(&u) {
                hits.push(t);
            }
        }
    }
    hits.sort_by(f64::total_cmp);
    hits.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    hits.first().zip(hits.last()).map(|(a, b)| [*a, *b])
}

/// Material geometry of one panel at a printed scale. Purely geometric: the
/// solver, the rest mesh and the stored contour are never touched.
pub fn sheet_geometry(panel: &Panel, scale: PrintScale) -> Result<SheetGeometry, String> {
    let mm = scale.millimetres_per_metre();
    let (sewing, cutting) = match panel.polygon.as_ref() {
        Some(polygon) => (sewing_rings(polygon)?, cutting_rings(polygon)?),
        // A legacy rectangular panel has no draft markings.
        None => {
            let w = panel.pattern.width;
            let h = panel.pattern.height;
            let rect = vec![
                DVec2::ZERO,
                DVec2::new(w, 0.),
                DVec2::new(w, h),
                DVec2::new(0., h),
            ];
            (vec![rect.clone()], vec![rect])
        }
    };
    let has_allowance = panel
        .polygon
        .as_ref()
        .is_some_and(|p| p.seam_allowances.values().any(|w| *w > 0.));
    let mut notches = vec![];
    let mut lines = vec![];
    if let Some(polygon) = panel.polygon.as_ref() {
        for name in polygon.notches.keys() {
            notches.extend(polygon.notch_segments(name).unwrap_or_default());
        }
        for line in &polygon.lines {
            lines.push(line.samples().unwrap_or_default());
        }
    }
    let grain = {
        let centre = DVec2::new(panel.pattern.width, panel.pattern.height) * 0.5;
        let angle = panel.grain_degrees.to_radians();
        let direction = DVec2::new(angle.cos(), angle.sin());
        span(&cutting, centre, direction)
            .map(|[a, b]| vec![centre + direction * a, centre + direction * b])
            .unwrap_or_default()
    };
    // The sheet origin is the cutting contour's own minimum, so a piece with an
    // outward allowance still starts at the page margin instead of at the paper
    // edge, and the label strip stays clear of the geometry.
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for p in cutting.iter().flatten() {
        lo = lo.min(*p * mm);
        hi = hi.max(*p * mm);
    }
    let extent = hi - lo + DVec2::splat(2. * MARGIN_MM);
    if !extent.is_finite() || extent.min_element() <= 0. {
        return Err("A printed sheet needs a non-degenerate contour".into());
    }
    Ok(SheetGeometry {
        cutting: cutting
            .iter()
            .map(|r| r.iter().map(|p| *p * mm).collect())
            .collect(),
        sewing: sewing
            .iter()
            .map(|r| r.iter().map(|p| *p * mm).collect())
            .collect(),
        notches: notches.iter().map(|[a, b]| [*a * mm, *b * mm]).collect(),
        lines: lines
            .iter()
            .map(|l| l.iter().map(|p| *p * mm).collect())
            .collect(),
        grain: grain.iter().map(|p| *p * mm).collect(),
        origin_mm: lo,
        extent_mm: extent,
        has_allowance,
    })
}

fn sewing_rings(polygon: &PolygonPattern) -> Result<Vec<Vec<DVec2>>, String> {
    let mut rings = vec![polygon.outer_points()?];
    rings.extend(polygon.holes.iter().map(|h| h.points.clone()));
    Ok(rings)
}

fn cutting_rings(polygon: &PolygonPattern) -> Result<Vec<Vec<DVec2>>, String> {
    match polygon.cutting_outline()? {
        Some(rings) => Ok(rings),
        None => sewing_rings(polygon),
    }
}

/// One printable page: the paper it targets, its place in the panel's tile grid
/// and the SVG document itself.
#[derive(Clone, Debug)]
pub struct Sheet {
    pub panel: String,
    pub column: usize,
    pub row: usize,
    pub columns: usize,
    pub rows: usize,
    pub name: String,
    pub svg: String,
}

/// Corner registration cross, drawn in paper millimetres relative to the page.
fn registration(out: &mut String, at: DVec2) {
    let arm = 4.;
    write!(
        out,
        "<path d=\"M {} {} L {} {} M {} {} L {} {} M {} {} L {} {}\" stroke=\"#000\" stroke-width=\"0.3\" fill=\"none\"/>",
        at.x - arm,
        at.y,
        at.x + arm,
        at.y,
        at.x,
        at.y - arm,
        at.x,
        at.y + arm,
        at.x - 1.5,
        at.y - 1.5,
        at.x + 1.5,
        at.y + 1.5
    )
    .unwrap();
}

/// Millimetre ruler: a straight baseline with ticks, so a print can be checked
/// before any fabric is cut. Always exactly [`RULER_MM`] long.
fn ruler(out: &mut String, origin: DVec2) {
    let end = origin + DVec2::new(RULER_MM, 0.);
    write!(
        out,
        "<path d=\"M {} {} L {} {}\" stroke=\"#000\" stroke-width=\"0.4\" fill=\"none\"/>",
        origin.x, origin.y, end.x, end.y
    )
    .unwrap();
    let mut x = 0.;
    while x <= RULER_MM + 1e-9 {
        let long = x % 50. == 0.;
        let at = origin + DVec2::new(x, 0.);
        write!(
            out,
            "<path d=\"M {} {} L {} {}\" stroke=\"#000\" stroke-width=\"0.3\" fill=\"none\"/>",
            at.x,
            at.y,
            at.x,
            at.y + if long { 6. } else { 3. }
        )
        .unwrap();
        if long {
            write!(
                out,
                "<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"3\" fill=\"#000\">{:.0}</text>",
                at.x, at.y + 10., x
            )
            .unwrap();
        }
        x += RULER_TICK_MM;
    }
    write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"3.5\" fill=\"#000\">{} mm check</text>",
        origin.x,
        origin.y - 2.,
        RULER_MM as i32
    )
    .unwrap();
}

fn polyline(out: &mut String, points: &[DVec2]) {
    let mut first = true;
    for p in points {
        if !first {
            out.push(' ');
        }
        let _ = write!(out, "{:.3},{:.3}", p.x, p.y);
        first = false;
    }
}

/// Build every printable sheet of a template: one page per tile of every panel,
/// in panel order and row-major tile order.
pub fn pattern_sheets(
    template: &GarmentTemplate,
    paper: Paper,
    scale: PrintScale,
) -> Result<Vec<Sheet>, String> {
    let (page_w, page_h) = paper.millimetres();
    if template.panels.is_empty() {
        return Err("There is nothing to print".into());
    }
    let mut sheets = vec![];
    for panel in &template.panels {
        let geometry = sheet_geometry(panel, scale)?;
        // Tiles step by a page minus the overlap, so neighbours repeat a strip
        // that carries the same geometry and can be taped over it.
        let step_x = page_w - OVERLAP_MM;
        let step_y = page_h - OVERLAP_MM;
        let columns = ((geometry.extent_mm.x / step_x).ceil() as usize).max(1);
        let rows = ((geometry.extent_mm.y / step_y).ceil() as usize).max(1);
        if columns * rows > MAX_SHEETS {
            return Err(format!(
                "{} needs {} pages at {}; use a smaller print scale",
                panel.id,
                columns * rows,
                scale.label()
            ));
        }
        for row in 0..rows {
            for column in 0..columns {
                sheets.push(Sheet {
                    panel: panel.id.clone(),
                    column,
                    row,
                    columns,
                    rows,
                    name: panel.name.clone(),
                    svg: page_svg(
                        template, panel, &geometry, paper, scale, column, row, columns, rows,
                    ),
                });
            }
        }
    }
    Ok(sheets)
}

#[allow(clippy::too_many_arguments)]
fn page_svg(
    template: &GarmentTemplate,
    panel: &Panel,
    geometry: &SheetGeometry,
    paper: Paper,
    scale: PrintScale,
    column: usize,
    row: usize,
    columns: usize,
    rows: usize,
) -> String {
    let (page_w, page_h) = paper.millimetres();
    let step = DVec2::new(page_w - OVERLAP_MM, page_h - OVERLAP_MM);
    let shift = DVec2::new(column as f64, row as f64) * step;
    // The tile's paper origin is the sheet origin shifted by its grid offset;
    // the whole panel is drawn and the page rect clips it.
    let placed = |p: DVec2| p - geometry.origin_mm + DVec2::splat(MARGIN_MM) - shift;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{page_w}mm\" height=\"{page_h}mm\" viewBox=\"0 0 {page_w} {page_h}\">\
<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\
<clipPath id=\"page\"><rect x=\"0\" y=\"0\" width=\"{page_w}\" height=\"{page_h}\"/></clipPath>\
<g clip-path=\"url(#page)\">"
    );
    // Cutting contour: solid, heavier — this is the line the scissors follow.
    for ring in &geometry.cutting {
        if ring.len() < 2 {
            continue;
        }
        let mut path: Vec<DVec2> = ring.iter().map(|p| placed(*p)).collect();
        path.push(path[0]);
        out.push_str("<polyline points=\"");
        polyline(&mut out, &path);
        out.push_str("\" fill=\"none\" stroke=\"#111111\" stroke-width=\"0.8\"/>");
    }
    // Sewing contour: dashed and lighter, so it stays distinct from the cutting
    // line. Without an allowance the two contours are the same polyline, so the
    // single solid line above already is the sewing line and is not redrawn.
    if geometry.has_allowance {
        for ring in &geometry.sewing {
            if ring.len() < 2 {
                continue;
            }
            let mut path: Vec<DVec2> = ring.iter().map(|p| placed(*p)).collect();
            path.push(path[0]);
            out.push_str("<polyline points=\"");
            polyline(&mut out, &path);
            out.push_str("\" fill=\"none\" stroke=\"#1b6fb5\" stroke-width=\"0.35\" stroke-dasharray=\"3 1.5\"/>");
        }
    }
    for [base, tip] in &geometry.notches {
        out.push_str("<path d=\"M ");
        let _ = write!(
            out,
            "{:.3} {:.3} L {:.3} {:.3}",
            placed(*base).x,
            placed(*base).y,
            placed(*tip).x,
            placed(*tip).y
        );
        out.push_str("\" stroke=\"#111111\" stroke-width=\"0.6\" fill=\"none\"/>");
    }
    for line in &geometry.lines {
        if line.len() < 2 {
            continue;
        }
        out.push_str("<polyline points=\"");
        polyline(
            &mut out,
            &line.iter().map(|p| placed(*p)).collect::<Vec<_>>(),
        );
        out.push_str(
            "\" fill=\"none\" stroke=\"#8a2be2\" stroke-width=\"0.35\" stroke-dasharray=\"2 2\"/>",
        );
    }
    if geometry.grain.len() == 2 {
        let [a, b] = [placed(geometry.grain[0]), placed(geometry.grain[1])];
        let direction = (b - a).normalize_or_zero();
        let perp = DVec2::new(-direction.y, direction.x);
        let arrow = b - direction * 8.;
        let _ = write!(
            out,
            "<path d=\"M {:.3} {:.3} L {:.3} {:.3} M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3}\" stroke=\"#c0392b\" stroke-width=\"0.4\" fill=\"none\"/>",
            a.x,
            a.y,
            b.x,
            b.y,
            arrow.x + perp.x * 3.,
            arrow.y + perp.y * 3.,
            b.x,
            b.y,
            arrow.x - perp.x * 3.,
            arrow.y - perp.y * 3.
        );
    }
    out.push_str("</g>");
    // Registration and labels sit outside the clipped group, on the paper.
    for corner in [
        DVec2::new(MARGIN_MM * 0.5, MARGIN_MM * 0.5),
        DVec2::new(page_w - MARGIN_MM * 0.5, MARGIN_MM * 0.5),
        DVec2::new(MARGIN_MM * 0.5, page_h - MARGIN_MM * 0.5),
        DVec2::new(page_w - MARGIN_MM * 0.5, page_h - MARGIN_MM * 0.5),
    ] {
        registration(&mut out, corner);
    }
    ruler(&mut out, DVec2::new(MARGIN_MM, page_h - 24.));
    let summary = if geometry.has_allowance {
        "cutting line solid · sewing line dashed"
    } else {
        "no seam allowance: sewing and cutting lines coincide"
    };
    let _ = write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"4\" fill=\"#000\">{} · {}</text>\
<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"4\" fill=\"#000\">{} · {} · page {}/{} (col {}, row {}) · {}</text>\
<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"3.2\" fill=\"#444\">{}</text>\
<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"3.2\" fill=\"#444\">Cut on the solid line; sew on the dashed line. Overlap the marked strip when taping sheets.</text>",
        MARGIN_MM,
        MARGIN_MM * 0.8,
        xml_escape(&panel.name),
        xml_escape(&panel.id),
        MARGIN_MM,
        MARGIN_MM * 0.8 + 5.,
        scale.label(),
        paper.name(),
        row * columns + column + 1,
        columns * rows,
        column + 1,
        row + 1,
        summary,
        MARGIN_MM,
        page_h - 32.,
        xml_escape(&template.name),
        MARGIN_MM,
        page_h - 28.,
    );
    out.push_str("</svg>");
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Write every sheet as `<directory>/<panel>-r<row>c<column>.svg`, returning the
/// paths. One file per page; the directory is created when missing.
pub fn write_sheets(
    template: &GarmentTemplate,
    directory: &FsPath,
    paper: Paper,
    scale: PrintScale,
) -> Result<Vec<PathBuf>, String> {
    let sheets = pattern_sheets(template, paper, scale)?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let mut paths = vec![];
    for sheet in &sheets {
        let path = directory.join(format!(
            "{}-r{}c{}.svg",
            sheet.panel,
            sheet.row + 1,
            sheet.column + 1
        ));
        fs::write(&path, &sheet.svg).map_err(|e| e.to_string())?;
        paths.push(path);
    }
    Ok(paths)
}
