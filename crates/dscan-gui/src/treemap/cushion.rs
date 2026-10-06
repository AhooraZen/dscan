/// Quadratic surface parameters for Van Wijk Cushion Treemap
/// Represents z(x, y) = (ax*x^2 + bx*x + cx) + (ay*y^2 + by*y + cy)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CushionSurface {
    pub ax: f32,
    pub bx: f32,
    pub cx: f32,
    pub ay: f32,
    pub by: f32,
    pub cy: f32,
}

impl Default for CushionSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl CushionSurface {
    pub const fn new() -> Self {
        Self {
            ax: 0.0,
            bx: 0.0,
            cx: 0.0,
            ay: 0.0,
            by: 0.0,
            cy: 0.0,
        }
    }

    /// Add a 2D parabolic ridge over the rectangle [x1, x2] x [y1, y2] with peak height `h`
    pub fn add_ridge(&mut self, x1: f32, x2: f32, y1: f32, y2: f32, h: f32) {
        let w = (x2 - x1).max(1.0);
        let h_box = (y2 - y1).max(1.0);

        let w_sq = w * w;
        let h_sq = h_box * h_box;

        self.ax -= 4.0 * h / w_sq;
        self.bx += 4.0 * h * (x1 + x2) / w_sq;
        self.cx -= 4.0 * h * x1 * x2 / w_sq;

        self.ay -= 4.0 * h / h_sq;
        self.by += 4.0 * h * (y1 + y2) / h_sq;
        self.cy -= 4.0 * h * y1 * y2 / h_sq;
    }

    /// Compute normal and lighting intensity at (x, y) with directional light (lx, ly, lz)
    pub fn compute_intensity(&self, x: f32, y: f32, lx: f32, ly: f32, lz: f32) -> f32 {
        let dzdx = 2.0 * self.ax * x + self.bx;
        let dzdy = 2.0 * self.ay * y + self.by;

        let nx = -dzdx;
        let ny = -dzdy;
        let nz = 1.0;

        let n_len = (nx * nx + ny * ny + nz * nz).sqrt().max(0.0001);
        let l_len = (lx * lx + ly * ly + lz * lz).sqrt().max(0.0001);

        let dot = (nx * lx + ny * ly + nz * lz) / (n_len * l_len);
        let cos_theta = dot.max(0.0);

        let ambient = 0.20;
        let diffuse = 0.80;
        (ambient + diffuse * cos_theta).clamp(0.0, 1.0)
    }

    /// Compute shaded RGB color given base color and rectangle bounds
    pub fn shade_color(
        &self,
        base_color: gpui::Rgba,
        x1: f32,
        x2: f32,
        y1: f32,
        y2: f32,
    ) -> gpui::Rgba {
        // Sample light at top-left 30% to capture the cushion normal slope
        let sx = x1 + (x2 - x1) * 0.30;
        let sy = y1 + (y2 - y1) * 0.30;
        // Directional light from top-left: L = (-1.0, -1.0, 1.5)
        let intensity = self.compute_intensity(sx, sy, -1.0, -1.0, 1.5);

        gpui::Rgba {
            r: (base_color.r * intensity).clamp(0.0, 1.0),
            g: (base_color.g * intensity).clamp(0.0, 1.0),
            b: (base_color.b * intensity).clamp(0.0, 1.0),
            a: base_color.a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cushion_ridge_and_intensity() {
        let mut surface = CushionSurface::new();
        surface.add_ridge(0.0, 100.0, 0.0, 100.0, 0.5);

        // Center should have 0 gradient (flat top)
        let dzdx_center = 2.0 * surface.ax * 50.0 + surface.bx;
        let dzdy_center = 2.0 * surface.ay * 50.0 + surface.by;
        assert!(dzdx_center.abs() < 1e-4);
        assert!(dzdy_center.abs() < 1e-4);

        // Intensity at center with light (-1, -1, 2)
        let intensity_center = surface.compute_intensity(50.0, 50.0, -1.0, -1.0, 2.0);
        assert!(intensity_center > 0.3 && intensity_center <= 1.0);

        // Shade color should scale components
        let base = gpui::rgb(0x38BDF8); // Sky 400
        let shaded = surface.shade_color(base, 0.0, 100.0, 0.0, 100.0);
        assert!(shaded.r <= base.r + 1e-4);
        assert!(shaded.g <= base.g + 1e-4);
        assert!(shaded.b <= base.b + 1e-4);
    }
}
