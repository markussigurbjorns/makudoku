use std::{fmt::Write, usize};

use crate::{Constraint, NN, col_of, row_of};

#[derive(Clone, Copy, PartialEq)]
pub enum Layer {
    UnderGrid,
    UnderDigits,
    OverDigits,
}

#[derive(Clone, Copy)]
pub struct RenderOptions<'a> {
    pub cell_size: f32,
    pub padding: f32,
    pub stroke_thin: f32,
    pub stroke_bold: f32,
    pub font_family: &'a str,
    pub font_size: f32,
    pub kropki_radius: f32,
    pub thermo_bulb_radius: f32,
    pub thermo_line_w: f32,
    pub thermo_corner_radius: f32,
    pub arrow_circle_radius: f32,
}

impl<'a> Default for RenderOptions<'a> {
    fn default() -> Self {
        Self {
            cell_size: 50.0,
            padding: 10.0,
            stroke_thin: 1.0,
            stroke_bold: 3.0,
            font_family: "monospace",
            font_size: 26.0,
            kropki_radius: 7.0,
            thermo_bulb_radius: 15.0,
            thermo_line_w: 9.0,
            thermo_corner_radius: 5.0,
            arrow_circle_radius: 15.0,
        }
    }
}

struct Layout {
    cell: f32,
    pad: f32,
}

impl Layout {
    fn new(opts: &RenderOptions) -> Self {
        Self {
            cell: opts.cell_size,
            pad: opts.padding,
        }
    }

    fn width(&self) -> f32 {
        self.pad * 2.0 + self.cell * 9.0
    }

    fn height(&self) -> f32 {
        self.width()
    }

    fn cell_origin(&self, row: usize, col: usize) -> (f32, f32) {
        (
            self.pad + self.cell * col as f32,
            self.pad + self.cell * row as f32,
        )
    }

    fn cell_center(&self, row: usize, col: usize) -> (f32, f32) {
        let (x, y) = self.cell_origin(row, col);
        (x + self.cell / 2.0, y + self.cell / 2.0)
    }
}

struct SvgDoc {
    buf: String,
}

impl SvgDoc {
    fn new() -> Self {
        Self { buf: String::new() }
    }

    fn finish(self) -> String {
        self.buf
    }
}

pub fn render_puzzle_svg(
    puzzle: &str,
    constraints: &[Constraint],
    opts: RenderOptions,
) -> Result<String, String> {
    let bytes: Vec<u8> = puzzle
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .map(|ch| ch as u8)
        .collect();
    if bytes.len() != NN {
        return Err(format!(
            "expected 81 chars (ignoring whitespace), got {}",
            bytes.len()
        ));
    }

    let layout = Layout::new(&opts);
    let mut svg = SvgDoc::new();

    writeln!(
        svg.buf,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = layout.width(),
        h = layout.height()
    )
    .unwrap();

    draw_constraints(&layout, &opts, constraints, Layer::UnderGrid, &mut svg)?;
    draw_grid(&layout, &opts, &mut svg);
    draw_constraints(&layout, &opts, constraints, Layer::UnderDigits, &mut svg)?;
    draw_givens(&layout, &opts, &bytes, &mut svg);
    draw_constraints(&layout, &opts, constraints, Layer::OverDigits, &mut svg)?;

    svg.buf.push_str("</svg>");
    Ok(svg.finish())
}

fn draw_grid(layout: &Layout, opts: &RenderOptions, svg: &mut SvgDoc) {
    for i in 1..9 {
        let pos = layout.pad + layout.cell * i as f32;

        writeln!(
            svg.buf,
            r#"<line x1="{x}" y1="{pad}" x2="{x}" y2="{ymax}" stroke="black" stroke-width="{w}" />"#,
            x = pos,
            pad = layout.pad,
            ymax = layout.height() - layout.pad,
            w = opts.stroke_thin
        )
        .unwrap();

        writeln!(
            svg.buf,
            r#"<line x1="{pad}" y1="{y}" x2="{xmax}" y2="{y}" stroke="black" stroke-width="{w}" />"#,
            pad = layout.pad,
            xmax = layout.width() - layout.pad,
            y = pos,
            w = opts.stroke_thin
        )
        .unwrap();
    }
    for i in 1..3 {
        let pos = layout.pad + layout.cell * (i as f32 * 3.0);

        writeln!(
            svg.buf,
            r#"<line x1="{x}" y1="{pad}" x2="{x}" y2="{ymax}" stroke="black" stroke-width="{w}" />"#,
            x = pos,
            pad = layout.pad,
            ymax = layout.height() - layout.pad,
            w = opts.stroke_bold
        )
        .unwrap();

        writeln!(
            svg.buf,
            r#"<line x1="{pad}" y1="{y}" x2="{xmax}" y2="{y}" stroke="black" stroke-width="{w}" />"#,
            pad = layout.pad,
            xmax = layout.width() - layout.pad,
            y = pos,
            w = opts.stroke_bold
        )
        .unwrap();
    }

    let x = layout.pad;
    let y = layout.pad;
    let w = layout.width() - 2.0 * layout.pad;
    let h = layout.height() - 2.0 * layout.pad;

    writeln!(
        svg.buf,
        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="none" stroke="black" stroke-width="{sw}" />"#,
        sw = opts.stroke_bold,
    )
    .unwrap();
}

fn draw_givens(layout: &Layout, opts: &RenderOptions, bytes: &[u8], svg: &mut SvgDoc) {
    for (i, &ch) in bytes.iter().enumerate() {
        if !(b'1'..=b'9').contains(&ch) {
            continue;
        }
        let row = i / 9;
        let col = i % 9;
        let (cx, cy) = layout.cell_center(row, col);
        writeln!(
            svg.buf,
            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" font-family="{font}" font-size="{size}">{digit}</text>"#,
            x = cx,
            y = cy + opts.font_size * 0.08,
            font = opts.font_family,
            size = opts.font_size,
            digit = ch as char
        )
        .unwrap();
    }
}

fn draw_constraints(
    layout: &Layout,
    opts: &RenderOptions,
    constraints: &[Constraint],
    layer: Layer,
    svg: &mut SvgDoc,
) -> Result<(), String> {
    for c in constraints {
        if constraint_layer(c) != layer {
            continue;
        }
        match c {
            Constraint::AllDifferent { .. } => {}
            Constraint::KropkiWhite { a, b } => draw_kropki(layout, opts, *a, *b, false, svg),
            Constraint::KropkiBlack { a, b } => draw_kropki(layout, opts, *a, *b, true, svg),
            Constraint::Thermo { cells, len } => draw_thermo(layout, opts, cells, *len, svg)?,
            Constraint::Arrow { cells, len } => draw_arrow(layout, opts, cells, *len, svg)?,
        }
    }
    Ok(())
}

fn constraint_layer(c: &Constraint) -> Layer {
    match c {
        Constraint::AllDifferent { .. } => Layer::UnderGrid,
        Constraint::Thermo { .. } => Layer::UnderGrid,
        Constraint::KropkiWhite { .. } | Constraint::KropkiBlack { .. } => Layer::OverDigits,
        Constraint::Arrow { .. } => Layer::UnderGrid,
    }
}

fn draw_kropki(layout: &Layout, opts: &RenderOptions, a: u8, b: u8, black: bool, svg: &mut SvgDoc) {
    let (ra, ca) = (row_of(a), col_of(a));
    let (rb, cb) = (row_of(b), col_of(b));
    let (xa, ya) = layout.cell_center(ra, ca);
    let (xb, yb) = layout.cell_center(rb, cb);
    let (mx, my) = ((xa + xb) / 2.0, (ya + yb) / 2.0);

    let fill = if black { "black" } else { "white" };
    writeln!(
        svg.buf,
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" stroke="black" stroke-width="{w}" />"#,
        x = mx,
        y = my,
        r = opts.kropki_radius,
        fill = fill,
        w = opts.stroke_thin
    )
    .unwrap();
}

fn draw_thermo(
    layout: &Layout,
    opts: &RenderOptions,
    cells: &[u8; 9],
    len: u8,
    svg: &mut SvgDoc,
) -> Result<(), String> {
    if len == 0 {
        return Ok(());
    }
    let len = len as usize;

    let (r0, c0) = (row_of(cells[0]), col_of(cells[0]));
    let (x0, y0) = layout.cell_center(r0, c0);
    writeln!(
        svg.buf,
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="gainsboro" stroke="gainsboro" stroke-width="{w}" />"#,
        x = x0,
        y = y0,
        r = opts.thermo_bulb_radius,
        w = opts.stroke_bold
    )
    .unwrap();

    let mut points = Vec::with_capacity(len);
    for &cell in cells.iter().take(len) {
        let (r, c) = (row_of(cell), col_of(cell));
        points.push(layout.cell_center(r, c));
    }

    let d = rounded_polyline_path(&points, opts.thermo_corner_radius);

    writeln!(
        svg.buf,
        r#"<path d="{d}" fill="none" stroke="gainsboro" stroke-linecap="round" stroke-linejoin="round" stroke-width="{w}" />"#,
        d = d,
        w = opts.thermo_line_w
    )
    .unwrap();

    Ok(())
}

fn draw_arrow(
    layout: &Layout,
    opts: &RenderOptions,
    cells: &[u8; 9],
    len: u8,
    svg: &mut SvgDoc,
) -> Result<(), String> {
    if len == 0 {
        return Ok(());
    }
    let len = len as usize;
    let (r0, c0) = (row_of(cells[0]), col_of(cells[0]));
    let (x0, y0) = layout.cell_center(r0, c0);
    writeln!(
        svg.buf,
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="none" stroke="gray" stroke-width="{w}" />"#,
        x = x0,
        y = y0,
        r = opts.arrow_circle_radius,
        w = opts.stroke_bold
    )
    .unwrap();

    if len < 2 {
        return Ok(());
    }

    let mut points = Vec::with_capacity(len);
    for &cell in cells.iter().take(len) {
        let (r, c) = (row_of(cell), col_of(cell));
        points.push(layout.cell_center(r, c));
    }

    let start = points[0];
    let first = points[1];
    let dir = (first.0 - start.0, first.1 - start.1);
    let dir_len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    let mut path_points = Vec::with_capacity(points.len());
    if dir_len > 1e-3 {
        let scale = opts.arrow_circle_radius / dir_len;
        let start_on_circle = (start.0 + dir.0 * scale, start.1 + dir.1 * scale);
        path_points.push(start_on_circle);
    } else {
        path_points.push(start);
    }
    path_points.extend_from_slice(&points[1..]);

    let mut d = String::new();
    let (sx, sy) = path_points[0];
    d.push_str(&format!("M {sx} {sy}"));

    for &(x, y) in &path_points[1..] {
        d.push_str(&format!(" L {x} {y}"));
    }

    writeln!(
        svg.buf,
        r#"<path d="{d}" fill="none" stroke="gray" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round" />"#,
        d = d,
        w = opts.stroke_bold,
    )
    .unwrap();

    if path_points.len() >= 2 {
        let tip = *path_points.last().unwrap();
        let prev = path_points[path_points.len() - 2];
        let dir = (tip.0 - prev.0, tip.1 - prev.1);
        let dir_len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
        if dir_len > 1e-3 {
            let ux = dir.0 / dir_len;
            let uy = dir.1 / dir_len;

            let angle = std::f32::consts::FRAC_PI_4;
            let sin = angle.sin();
            let cos = angle.cos();

            let head_len = layout.cell * 0.2;

            let left = (
                tip.0 - (ux * cos - uy * sin) * head_len,
                tip.1 - (ux * sin + uy * cos) * head_len,
            );
            let right = (
                tip.0 - (ux * cos + uy * sin) * head_len,
                tip.1 - (-ux * sin + uy * cos) * head_len,
            );

            writeln!(
                svg.buf,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="gray" stroke-width="{w}" stroke-linecap="round" />"#,
                x1 = tip.0,
                y1 = tip.1,
                x2 = left.0,
                y2 = left.1,
                w = opts.stroke_bold
            )
            .unwrap();
            writeln!(
                svg.buf,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="gray" stroke-width="{w}" stroke-linecap="round" />"#,
                x1 = tip.0,
                y1 = tip.1,
                x2 = right.0,
                y2 = right.1,
                w = opts.stroke_bold
            )
            .unwrap();
        }
    }

    Ok(())
}

fn rounded_polyline_path(points: &[(f32, f32)], radius: f32) -> String {
    use std::fmt::Write as _;

    let mut d = String::new();
    if points.is_empty() {
        return d;
    }

    write!(&mut d, "M {} {} ", points[0].0, points[0].1).unwrap();

    if points.len() == 1 {
        return d.trim_end().to_string();
    }

    if radius <= 0.0 || points.len() == 2 {
        for p in &points[1..] {
            write!(&mut d, "L {} {} ", p.0, p.1).unwrap();
        }
        return d.trim_end().to_string();
    }

    let mut prev = points[0];

    for i in 1..points.len() {
        let is_last = i == points.len() - 1;

        if is_last {
            let p_last = points[i];
            write!(&mut d, "L {} {} ", p_last.0, p_last.1).unwrap();
            break;
        }

        let curr = points[i];
        let next = points[i + 1];

        let v1 = (curr.0 - prev.0, curr.1 - prev.1);
        let v2 = (next.0 - curr.0, next.1 - curr.1);

        let len1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt();
        let len2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt();

        if len1 < 1e-3 || len2 < 1e-3 {
            write!(&mut d, "L {} {} ", curr.0, curr.1).unwrap();
            prev = curr;
            continue;
        }

        let d1 = (v1.0 / len1, v1.1 / len1);
        let d2 = (v2.0 / len2, v2.1 / len2);

        let mut dot = (-d1.0) * d2.0 + (-d1.1) * d2.1;
        if dot > 1.0 {
            dot = 1.0;
        } else if dot < -1.0 {
            dot = -1.0;
        }
        let theta = dot.acos();

        if theta.abs() < 1e-3 {
            write!(&mut d, "L {} {} ", curr.0, curr.1).unwrap();
            prev = curr;
            continue;
        }

        let mut offset = radius / (theta / 2.0).tan();
        let max_off1 = len1 * 0.5;
        let max_off2 = len2 * 0.5;
        if offset > max_off1 {
            offset = max_off1;
        }
        if offset > max_off2 {
            offset = max_off2;
        }

        let enter = (curr.0 - d1.0 * offset, curr.1 - d1.1 * offset);

        let exit = (curr.0 + d2.0 * offset, curr.1 + d2.1 * offset);

        write!(&mut d, "L {} {} ", enter.0, enter.1).unwrap();
        write!(&mut d, "Q {} {} {} {} ", curr.0, curr.1, exit.0, exit.1).unwrap();

        prev = exit;
    }

    d.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Constraint, types::idx};

    #[test]
    fn render_basic_svg() {
        let puzzle = ".".repeat(81);
        let mut constraints = Vec::new();
        constraints.push(Constraint::KropkiWhite { a: 0, b: 1 });
        constraints.push(Constraint::KropkiBlack { a: 9, b: 18 });
        let mut thermo_cells = [0u8; 9];
        thermo_cells[0] = idx(0, 0);
        thermo_cells[1] = idx(0, 1);
        thermo_cells[2] = idx(0, 2);
        constraints.push(Constraint::Thermo {
            cells: thermo_cells,
            len: 3,
        });

        let svg = render_puzzle_svg(&puzzle, &constraints, RenderOptions::default()).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("<circle"));
    }
}
