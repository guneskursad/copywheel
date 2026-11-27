//! Drawing primitives for the wheel and its card.

use super::theme::Palette;
use super::*;
use egui::emath::GuiRounding;

const SECTOR_GAP: f32 = 10.0;
const ARC_SAMPLE_LENGTH: f32 = 2.0;

pub(super) fn arc_points(
    center: Pos2,
    radius: f32,
    angles: std::ops::RangeInclusive<f32>,
) -> Vec<Pos2> {
    let samples =
        (((angles.end() - angles.start()) * radius / ARC_SAMPLE_LENGTH).ceil() as usize).max(1);
    (0..=samples)
        .map(|i| {
            center
                + egui::Vec2::angled(egui::lerp(angles.clone(), i as f32 / samples as f32)) * radius
        })
        .collect()
}

pub(super) struct Sector {
    outer: Vec<Pos2>,
    inner: Vec<Pos2>,
}

impl Sector {
    pub(super) fn new(center: Pos2, inner: f32, outer: f32, start: f32, end: f32) -> Self {
        // Intersect both circles with the same offset lines. Each neighbor owns half the gap.
        let offset = |radius: f32| (SECTOR_GAP / (2.0 * radius)).asin();
        let inside = offset(inner);
        let outside = offset(outer);
        let outer = arc_points(center, outer, start + outside..=end - outside);
        let samples = outer.len() - 1;
        let inner = (0..=samples)
            .map(|i| {
                let angle = egui::lerp(start + inside..=end - inside, i as f32 / samples as f32);
                center + egui::Vec2::angled(angle) * inner
            })
            .collect();
        Self { outer, inner }
    }

    pub(super) fn fill(&self, painter: &egui::Painter, color: Color32) {
        // The annular sector is concave. Tessellate it as a strip of quads.
        let mut mesh = egui::Mesh::default();
        for (&outer, &inner) in self.outer.iter().zip(&self.inner) {
            mesh.colored_vertex(outer, color);
            mesh.colored_vertex(inner, color);
        }
        for i in (0..mesh.vertices.len() as u32 - 2).step_by(2) {
            mesh.add_triangle(i, i + 2, i + 1);
            mesh.add_triangle(i + 1, i + 2, i + 3);
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    fn outline(&self) -> Vec<Pos2> {
        self.outer
            .iter()
            .chain(self.inner.iter().rev())
            .copied()
            .collect()
    }

    pub(super) fn stroke(&self, painter: &egui::Painter, stroke: Stroke) {
        painter.add(egui::Shape::closed_line(self.outline(), stroke));
    }

    pub(super) fn glow(&self, painter: &egui::Painter, accent: Color32, opacity: f32) {
        let outline = self.outline();
        let mut path = egui::epaint::tessellator::Path::default();
        path.add_line_loop(&outline);

        // Feather the two halo strokes into their surroundings before painting the crisp edge.
        let mut halo = egui::Mesh::default();
        for (width, alpha) in [(12.0_f32, 30), (6.0, 70)] {
            let color = accent.gamma_multiply_u8(alpha).gamma_multiply(opacity);
            path.stroke_closed(width / 2.0, &Stroke::new(width, color).into(), &mut halo);
        }
        painter.add(egui::Shape::mesh(halo));
        painter.add(egui::Shape::closed_line(
            outline,
            Stroke::new(2.5_f32, accent.gamma_multiply(opacity)),
        ));
    }
}

/// Limit a centered, horizontal label to the arcs and straight sides around it.
pub(super) fn sector_label_width(
    offset: egui::Vec2,
    half_height: f32,
    inner: f32,
    outer: f32,
    mid: f32,
    step: f32,
) -> f32 {
    let x_at = |radius: f32, y: f32| (radius.powi(2) - y.powi(2)).max(0.0).sqrt();
    let edge = x_at(outer, offset.y.abs() + half_height);
    let hole = x_at(inner, (offset.y.abs() - half_height).max(0.0));
    let mut half_width = edge - offset.x.abs();
    if hole > 0.0 {
        half_width = half_width.min(offset.x.abs() - hole);
    }
    if step <= PI {
        let first = mid - step / 2.0;
        let last = mid + step / 2.0;
        for normal in [
            vec2(-first.sin(), first.cos()),
            vec2(last.sin(), -last.cos()),
        ] {
            if normal.x.abs() > 0.001 {
                let room = normal.dot(offset) - normal.y.abs() * half_height - SECTOR_GAP / 2.0;
                half_width = half_width.min(room / normal.x.abs());
            }
        }
    }
    2.0 * half_width
}

/// A soft drop shadow under a rounded card.
pub(super) fn card_shadow(
    colors: &Palette,
    p: &egui::Painter,
    r: Rect,
    radius: u8,
    alpha: f32,
    light_strength: f32,
) {
    let shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: with_alpha(
            colors.card_shadow,
            if colors.dark_mode {
                alpha
            } else {
                alpha * light_strength
            },
        ),
    };
    let mut shape = shadow.as_shape(r, CornerRadius::same(radius));
    // epaint's inward feather nearly collapses the fill when the blur reaches the
    // short side. Leave room for the inner contour after pixel rounding.
    let short_side = shape
        .rect
        .round_to_pixels(p.pixels_per_point())
        .size()
        .min_elem();
    shape.blur_width = shape.blur_width.min((short_side - 2.0).max(0.0));
    p.add(shape);
}

/// Text drawn twice, a hair apart, so it reads as semi-bold.
pub(super) fn strong_galley(p: &egui::Painter, pos: Pos2, g: Arc<egui::Galley>, color: Color32) {
    p.galley(pos, g.clone(), color);
    p.galley(pos + vec2(0.55, 0.0), g, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_sides_keep_a_ten_point_gap_at_both_radii() {
        let center = pos2(0.0, 0.0);
        for n in 1..=PER_PAGE {
            let step = TAU / n as f32;
            let left = Sector::new(center, 78.0, 137.0, -step, 0.0);
            let right = Sector::new(center, 78.0, 137.0, 0.0, step);
            for (a, b) in [
                (left.outer.last().unwrap(), &right.outer[0]),
                (left.inner.last().unwrap(), &right.inner[0]),
            ] {
                assert!((a.x - b.x).abs() < 0.001);
                assert!((b.y - a.y - 10.0).abs() < 0.001);
            }
        }
    }
}
