//! Line icons in one style: thick strokes with round ends and joins, solid dots as
//! accents (bold, friendly, hand-made look). Drawn on a 24-unit grid and scaled to any size.

use super::*;

#[derive(Clone, Copy, PartialEq, Debug, Hash)]
pub(super) enum Icon {
    Close,
    Plus,
    Search,
    Star,
    StarFilled,
    Pencil,
    Trash,
    Copy,
    Pin,
    Clipboard,
    Check,
    Settings,
    Text,
    Link,
    Image,
    Right,
    Down,
    Back,
    Return,
}

/// Maps the 24-unit grid into `rect` and draws with round caps and joins.
struct Pen<'a> {
    p: &'a egui::Painter,
    o: Pos2,
    k: f32,
    col: Color32,
    w: f32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.o + vec2(x * self.k, y * self.k)
    }

    fn dot(&self, q: Pos2) {
        self.p.circle_filled(q, self.w / 2.0, self.col);
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        let (a, b) = (self.at(a.0, a.1), self.at(b.0, b.1));
        self.p.line_segment([a, b], Stroke::new(self.w, self.col));
        self.dot(a);
        self.dot(b);
    }

    fn poly(&self, pts: &[(f32, f32)], closed: bool) {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        for q in &pts {
            self.dot(*q);
        }
        let stroke = egui::epaint::PathStroke::new(self.w, self.col);
        let shape = if closed {
            egui::epaint::PathShape::closed_line(pts, stroke)
        } else {
            egui::epaint::PathShape::line(pts, stroke)
        };
        self.p.add(shape);
    }

    /// Filled shape that is star-shaped around its first point (a fan of triangles).
    fn fan(&self, centre: (f32, f32), pts: &[(f32, f32)]) {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(self.at(centre.0, centre.1), self.col);
        for &(x, y) in pts {
            mesh.colored_vertex(self.at(x, y), self.col);
        }
        let n = pts.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        self.p.add(egui::Shape::mesh(mesh));
    }

    fn circle(&self, c: (f32, f32), r: f32) {
        self.p.circle_stroke(self.at(c.0, c.1), r * self.k, Stroke::new(self.w, self.col));
    }

    fn disc(&self, c: (f32, f32), r: f32, col: Color32) {
        self.p.circle_filled(self.at(c.0, c.1), r * self.k, col);
    }

    fn rrect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        let cr = CornerRadius::same((r * self.k).round().clamp(0.0, 255.0) as u8);
        self.p.rect_stroke(rect, cr, Stroke::new(self.w, self.col), StrokeKind::Middle);
    }

    fn rrect_filled(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        let cr = CornerRadius::same((r * self.k).round().clamp(0.0, 255.0) as u8);
        self.p.rect_filled(rect, cr, self.col);
    }

}

fn arc_points(c: (f32, f32), r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
    let n = (((a1 - a0).abs() / 0.12).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

fn star_points(c: (f32, f32), r_out: f32, r_in: f32) -> Vec<(f32, f32)> {
    (0..10)
        .map(|i| {
            let a = -PI / 2.0 + i as f32 * PI / 5.0;
            let r = if i % 2 == 0 { r_out } else { r_in };
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

/// Stroke width for an icon `size` points tall: bold, never thinner than 1.5 pt.
pub(super) fn stroke_for(size: f32) -> f32 {
    (size * 0.092).max(1.5)
}

/// Draws `icon` centred in `rect` (it fits the shorter side) in `col`.
pub(super) fn paint_icon(p: &egui::Painter, icon: Icon, rect: Rect, col: Color32) {
    let size = rect.width().min(rect.height());
    let k = size / 24.0;
    let o = rect.center() - vec2(12.0 * k, 12.0 * k);
    let pen = Pen { p, o, k, col, w: stroke_for(size) };
    match icon {
        Icon::Close => {
            pen.line((6.5, 6.5), (17.5, 17.5));
            pen.line((17.5, 6.5), (6.5, 17.5));
        }
        Icon::Plus => {
            pen.line((12.0, 5.0), (12.0, 19.0));
            pen.line((5.0, 12.0), (19.0, 12.0));
        }
        Icon::Search => {
            pen.circle((10.5, 10.5), 6.5);
            pen.line((15.5, 15.5), (19.5, 19.5));
        }
        Icon::Star => {
            pen.poly(&star_points((12.0, 12.8), 9.2, 4.4), true);
        }
        Icon::StarFilled => {
            let pts = star_points((12.0, 12.8), 9.2, 4.4);
            pen.fan((12.0, 12.8), &pts);
            pen.poly(&pts, true);
        }
        Icon::Pencil => {
            pen.poly(&[(4.5, 19.5), (5.5, 15.0), (15.5, 5.0), (19.0, 8.5), (9.0, 18.5)], true);
            pen.line((13.2, 7.3), (16.7, 10.8));
        }
        Icon::Trash => {
            pen.line((4.5, 7.0), (19.5, 7.0));
            pen.poly(&[(9.5, 7.0), (9.5, 4.5), (14.5, 4.5), (14.5, 7.0)], false);
            pen.poly(&[(6.5, 7.0), (7.5, 19.5), (16.5, 19.5), (17.5, 7.0)], false);
            pen.line((10.5, 11.0), (10.5, 15.5));
            pen.line((13.5, 11.0), (13.5, 15.5));
        }
        Icon::Copy => {
            // the back sheet shows only where the front one doesn't cover it
            // (with a small gap before it, like two cards apart)
            let mut back = vec![(7.5, 14.0)];
            back.extend(arc_points((6.0, 11.5), 2.5, PI / 2.0, PI));
            back.extend(arc_points((6.0, 6.0), 2.5, PI, 1.5 * PI));
            back.extend(arc_points((11.5, 6.0), 2.5, 1.5 * PI, TAU));
            back.push((14.0, 7.5));
            pen.poly(&back, false);
            pen.rrect(10.0, 10.0, 20.5, 20.5, 2.5);
        }
        Icon::Pin => {
            pen.line((7.5, 4.0), (16.5, 4.0));
            pen.poly(&[(9.5, 4.0), (9.5, 10.0), (6.0, 14.5), (18.0, 14.5), (14.5, 10.0), (14.5, 4.0)], false);
            pen.line((12.0, 14.5), (12.0, 20.5));
        }
        Icon::Clipboard => {
            pen.rrect(5.0, 4.5, 19.0, 21.0, 3.0);
            pen.rrect_filled(8.5, 2.5, 15.5, 7.0, 1.8);
            pen.line((9.0, 12.0), (15.0, 12.0));
            pen.line((9.0, 16.0), (13.0, 16.0));
        }
        Icon::Settings => {
            let teeth: Vec<_> = (0..32).map(|i| {
                let angle = i as f32 * TAU / 32.0 - PI / 32.0;
                let radius = if i % 4 < 2 { 9.6 } else { 7.7 };
                (12.0 + radius * angle.cos(), 12.0 + radius * angle.sin())
            }).collect();
            pen.poly(&teeth, true);
            pen.circle((12.0, 12.0), 3.2);
        }
        Icon::Text => {
            pen.rrect(4.5, 2.5, 19.5, 21.5, 2.0);
            pen.line((8.0, 8.0), (16.0, 8.0));
            pen.line((8.0, 12.0), (16.0, 12.0));
            pen.line((8.0, 16.0), (13.0, 16.0));
        }
        Icon::Link => {
            pen.poly(&[(10.2, 7.0), (14.0, 3.4), (18.3, 3.4), (20.6, 5.7),
                (20.6, 10.0), (17.0, 13.8)], false);
            pen.poly(&[(13.8, 17.0), (10.0, 20.6), (5.7, 20.6), (3.4, 18.3),
                (3.4, 14.0), (7.0, 10.2)], false);
            pen.line((8.5, 15.5), (15.5, 8.5));
        }
        Icon::Image => {
            pen.rrect(2.5, 4.0, 21.5, 20.0, 3.0);
            pen.disc((8.0, 9.0), 1.5, col);
            pen.poly(&[(4.0, 17.0), (8.5, 12.8), (12.0, 16.0), (16.5, 11.5), (20.0, 15.0)], false);
        }
        Icon::Right => pen.poly(&[(9.0, 5.0), (15.0, 12.0), (9.0, 19.0)], false),
        Icon::Down => pen.poly(&[(5.0, 9.0), (12.0, 15.0), (19.0, 9.0)], false),
        Icon::Back => {
            pen.poly(&[(11.0, 5.0), (4.0, 12.0), (11.0, 19.0)], false);
            pen.line((4.0, 12.0), (20.0, 12.0));
        }
        Icon::Return => {
            pen.poly(&[(20.5, 5.0), (20.5, 14.0), (5.0, 14.0)], false);
            pen.poly(&[(10.5, 8.5), (5.0, 14.0), (10.5, 19.5)], false);
        }
        Icon::Check => {
            pen.poly(&[(5.0, 12.5), (10.0, 17.0), (19.0, 7.5)], false);
        }
    }
}

/// Allocates a small square and draws an icon in it (for labels such as the search field).
pub(super) fn icon_label(ui: &mut Ui, icon: Icon, size: f32, col: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), icon, rect, col);
}
