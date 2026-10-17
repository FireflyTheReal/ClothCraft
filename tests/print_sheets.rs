//! Physically scaled print sheets: 1 user unit = 1 mm, distinct cutting and
//! sewing lines, tiling with overlap and a check ruler. CPU-only.
use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command},
    garment::{
        GarmentTemplate,
        meshing::MeshingSettings,
        presets::{self, GarmentKind},
        print::{self, Paper, PrintScale, RULER_MM},
        shapes::Primitive,
    },
};

fn settings() -> MeshingSettings {
    MeshingSettings {
        target_edge_m: 0.05,
        ..Default::default()
    }
}

fn rectangle(w: f64, h: f64) -> GarmentTemplate {
    authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::new(w, h),
            sweep_degrees: 180.,
            origin: DVec2::ZERO,
            meshing: settings(),
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}

fn allowance(mut t: GarmentTemplate, edges: &[&str], width: f64) -> GarmentTemplate {
    for edge in edges {
        t = authoring::apply(
            &t,
            Command::SeamAllowance {
                panel: "panel_1".into(),
                edge: (*edge).into(),
                width_m: width,
            },
        )
        .unwrap()
        .template;
    }
    t
}

fn bounds(points: &[DVec2]) -> (DVec2, DVec2) {
    let lo = points
        .iter()
        .copied()
        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
    let hi = points
        .iter()
        .copied()
        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
    (lo, hi)
}

/// A 150 x 200 mm piece: with the 10 mm margin it fits a single A4 page at 1:1.
fn small() -> GarmentTemplate {
    rectangle(0.15, 0.2)
}

#[test]
fn one_page_is_declared_in_real_millimetres() {
    let t = small();
    let sheets = print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap();
    assert_eq!(sheets.len(), 1);
    let svg = &sheets[0].svg;
    assert!(svg.contains("width=\"210mm\" height=\"297mm\""), "{svg}");
    assert!(svg.contains("viewBox=\"0 0 210 297\""), "{svg}");
    let letter = print::pattern_sheets(&t, Paper::Letter, PrintScale::Full).unwrap();
    assert!(
        letter[0]
            .svg
            .contains("width=\"215.9mm\" height=\"279.4mm\"")
    );
}

#[test]
fn the_check_ruler_is_exactly_one_hundred_millimetres() {
    assert_eq!(RULER_MM, 100.);
    // A4 portrait: the ruler baseline starts at the left margin, 24 mm above
    // the bottom edge, and must therefore end 100 mm further right.
    let t = small();
    let svg = &print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap()[0].svg;
    assert!(svg.contains("M 10 273 L 110 273"), "ruler missing: {svg}");
}

#[test]
fn a_hundred_millimetre_edge_prints_one_hundred_units() {
    let t = rectangle(0.1, 0.2);
    let geometry = print::sheet_geometry(&t.panels[0], PrintScale::Full).unwrap();
    let (lo, hi) = bounds(&geometry.sewing[0]);
    assert!((lo - DVec2::ZERO).length() < 1e-9, "{lo:?}");
    assert!((hi - DVec2::new(100., 200.)).length() < 1e-9, "{hi:?}");
    // At 1:2 the same edge is 50 mm of paper.
    let half = print::sheet_geometry(&t.panels[0], PrintScale::Half).unwrap();
    let (_, hi) = bounds(&half.sewing[0]);
    assert!((hi - DVec2::new(50., 100.)).length() < 1e-9, "{hi:?}");
}

#[test]
fn the_cutting_and_sewing_contours_are_distinct_lines() {
    let t = allowance(small(), &["edge_1", "edge_2", "edge_3", "edge_4"], 0.01);
    let geometry = print::sheet_geometry(&t.panels[0], PrintScale::Full).unwrap();
    assert!(geometry.has_allowance);
    let (slo, shi) = bounds(&geometry.sewing[0]);
    let (clo, chi) = bounds(&geometry.cutting[0]);
    assert!((slo - DVec2::ZERO).length() < 1e-6);
    assert!((shi - DVec2::new(150., 200.)).length() < 1e-6);
    // A 10 mm allowance on every edge, square joined.
    assert!((clo - DVec2::splat(-10.)).length() < 1e-5, "{clo:?}");
    assert!((chi - DVec2::new(160., 210.)).length() < 1e-5, "{chi:?}");
    // The sheet origin is the cutting minimum, so the printed piece still starts
    // at the margin instead of at the paper edge, and the extent covers both.
    assert!(
        (geometry.origin_mm - DVec2::splat(-10.)).length() < 1e-5,
        "{:?}",
        geometry.origin_mm
    );
    assert!(
        (geometry.extent_mm - DVec2::new(190., 240.)).length() < 1e-5,
        "{:?}",
        geometry.extent_mm
    );
    let svg = &print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap()[0].svg;
    assert!(
        svg.contains("stroke-dasharray=\"3 1.5\""),
        "sewing line missing"
    );
    assert!(svg.contains("stroke-width=\"0.8\""), "cutting line missing");
    assert!(svg.contains("cutting line solid"), "{svg}");
}

#[test]
fn notches_are_printed_as_cut_marks() {
    let t = authoring::apply(
        &small(),
        Command::Notch {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            t: 0.5,
            depth_m: 0.006,
        },
    )
    .unwrap()
    .template;
    let geometry = print::sheet_geometry(&t.panels[0], PrintScale::Full).unwrap();
    assert_eq!(geometry.notches.len(), 1);
    let [base, tip] = geometry.notches[0];
    // Half of a 150 mm edge is 75 mm; the tick is 6 mm long and points out.
    assert!((base - DVec2::new(75., 0.)).length() < 1e-9, "{base:?}");
    assert!((tip - DVec2::new(75., -6.)).length() < 1e-9, "{tip:?}");
    let svg = &print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap()[0].svg;
    // Placed with the 10 mm margin, the tick runs from (85, 10) to (85, 4).
    assert!(
        svg.contains("M 85.000 10.000 L 85.000 4.000"),
        "notch tick missing: {svg}"
    );
}

#[test]
fn without_an_allowance_the_two_contours_coincide() {
    let t = small();
    let geometry = print::sheet_geometry(&t.panels[0], PrintScale::Full).unwrap();
    assert!(!geometry.has_allowance);
    assert_eq!(geometry.cutting, geometry.sewing);
    let svg = &print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap()[0].svg;
    assert!(svg.contains("coincide"), "{svg}");
    // One polygon, not two coincident ones: the dashed sewing line is omitted.
    assert_eq!(svg.matches("<polyline").count(), 1, "{svg}");
}

#[test]
fn a_large_panel_is_tiled_with_a_ten_millimetre_overlap() {
    let t = rectangle(1.0, 1.5);
    // A4 is 210 x 297 mm; the step is a page minus the 10 mm overlap.
    let sheets = print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap();
    assert_eq!(sheets[0].columns, 6);
    assert_eq!(sheets[0].rows, 6);
    assert_eq!(sheets.len(), 36);
    let first = sheets.iter().find(|s| s.column == 0 && s.row == 0).unwrap();
    let shifted = sheets.iter().find(|s| s.column == 1 && s.row == 0).unwrap();
    assert_ne!(first.svg, shifted.svg, "each tile must be its own page");
    assert!(first.svg.contains("page 1/36 (col 1, row 1)"));
    assert!(shifted.svg.contains("page 2/36 (col 2, row 1)"));
}

#[test]
fn a_curved_panel_with_an_allowance_prints() {
    let t = authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Primitive {
            kind: Primitive::Ellipse,
            size: DVec2::new(0.3, 0.2),
            sweep_degrees: 360.,
            origin: DVec2::ZERO,
            meshing: settings(),
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let t = allowance(t, &["edge_1", "edge_2", "edge_3", "edge_4"], 0.01);
    let geometry = print::sheet_geometry(&t.panels[0], PrintScale::Full).unwrap();
    assert!(geometry.has_allowance);
    assert!(geometry.cutting[0].len() > 32, "the curve is sampled");
    let sheets = print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap();
    assert!(!sheets.is_empty());
    assert!(sheets[0].svg.contains("stroke-dasharray=\"3 1.5\""));
}

#[test]
fn printing_is_deterministic() {
    let t = allowance(rectangle(0.4, 0.6), &["edge_1", "edge_2"], 0.01);
    let a = print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap();
    let b = print::pattern_sheets(&t, Paper::A4, PrintScale::Full).unwrap();
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.svg, y.svg);
    }
}

#[test]
fn an_impossible_page_count_is_refused() {
    let t = rectangle(5.0, 5.0);
    // 26 columns by 18 rows is 468 pages, past the ceiling.
    assert!(print::pattern_sheets(&t, Paper::A4, PrintScale::Full).is_err());
    // The same piece fits comfortably at 1:4.
    assert!(print::pattern_sheets(&t, Paper::A4, PrintScale::Quarter).is_ok());
}

#[test]
fn sheets_are_written_as_one_file_per_page() {
    let t = allowance(small(), &["edge_1"], 0.01);
    let dir = tempfile::tempdir().unwrap();
    let paths = print::write_sheets(&t, dir.path(), Paper::A4, PrintScale::Full).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].file_name().unwrap(), "panel_1-r1c1.svg");
    assert!(paths[0].exists());
    let svg = std::fs::read_to_string(&paths[0]).unwrap();
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
}
