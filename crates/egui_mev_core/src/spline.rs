//! Smooth curves through control points: the natural cubic spline Curves
//! uses (OpenPDN `SplineInterpolator`), and [`Curve`], a 0–1 → 0–1
//! response curve with a lookup table (tablet pressure, brush dynamics).

/// Natural cubic spline through `points` (Numerical Recipes §3.3), as
/// `SplineInterpolator`.
#[derive(Debug, Clone)]
pub struct Spline {
    xs: Vec<f64>,
    ys: Vec<f64>,
    y2: Vec<f64>,
}

impl Spline {
    /// Build from points (sorted and deduplicated by x).
    #[must_use]
    pub fn new(points: &[(f64, f64)]) -> Self {
        let mut pts = points.to_vec();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        pts.dedup_by(|a, b| a.0 == b.0);
        let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
        let n = xs.len();
        let mut y2 = vec![0.0; n];
        let mut u = vec![0.0; n];
        for i in 1..n.saturating_sub(1) {
            let wx = xs[i + 1] - xs[i - 1];
            let sig = (xs[i] - xs[i - 1]) / wx;
            let p = sig * y2[i - 1] + 2.0;
            y2[i] = (sig - 1.0) / p;
            let ddydx = (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i])
                - (ys[i] - ys[i - 1]) / (xs[i] - xs[i - 1]);
            u[i] = (6.0 * ddydx / wx - sig * u[i - 1]) / p;
        }
        for i in (0..n.saturating_sub(1)).rev() {
            y2[i] = y2[i] * y2[i + 1] + u[i];
        }
        Self { xs, ys, y2 }
    }

    /// Value at `x`.
    #[must_use]
    pub fn eval(&self, x: f64) -> f64 {
        let n = self.xs.len();
        match n {
            0 => return x,
            1 => return self.ys[0],
            _ => {}
        }
        let (mut lo, mut hi) = (0, n - 1);
        while hi - lo > 1 {
            let k = (hi + lo) / 2;
            if self.xs[k] > x {
                hi = k;
            } else {
                lo = k;
            }
        }
        let h = self.xs[hi] - self.xs[lo];
        let a = (self.xs[hi] - x) / h;
        let b = (x - self.xs[lo]) / h;
        a * self.ys[lo]
            + b * self.ys[hi]
            + ((a * a * a - a) * self.y2[lo] + (b * b * b - b) * self.y2[hi]) * (h * h) / 6.0
    }
}

/// Lookup table entries of a [`Curve`] (one more than the steps).
const CURVE_STEPS: usize = 256;

/// A response curve over 0–1: control points joined by a natural cubic
/// spline, clamped to 0–1, and tabulated so evaluating it is a lookup.
#[derive(Clone)]
pub struct Curve {
    points: Vec<[f32; 2]>,
    lut: Box<[f32]>,
}

impl Curve {
    /// Through `points` (each 0–1; sorted by x, duplicates dropped). Fewer
    /// than two points give the identity.
    #[must_use]
    pub fn new(points: &[[f32; 2]]) -> Self {
        let mut points: Vec<[f32; 2]> = points
            .iter()
            .map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)])
            .collect();
        points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        points.dedup_by(|a, b| a[0] == b[0]);
        if points.len() < 2 {
            points = vec![[0.0, 0.0], [1.0, 1.0]];
        }
        let spline = Spline::new(
            &points
                .iter()
                .map(|p| (f64::from(p[0]), f64::from(p[1])))
                .collect::<Vec<_>>(),
        );
        let (first, last) = (points[0], points[points.len() - 1]);
        let lut = (0..=CURVE_STEPS)
            .map(|i| {
                let x = i as f32 / CURVE_STEPS as f32;
                // Flat outside the first and last points.
                let y = if x <= first[0] {
                    first[1]
                } else if x >= last[0] {
                    last[1]
                } else {
                    spline.eval(f64::from(x)) as f32
                };
                y.clamp(0.0, 1.0)
            })
            .collect();
        Self { points, lut }
    }

    /// The identity.
    #[must_use]
    pub fn linear() -> Self {
        Self::new(&[[0.0, 0.0], [1.0, 1.0]])
    }

    /// Constant `value`.
    #[must_use]
    pub fn constant(value: f32) -> Self {
        Self::new(&[[0.0, value], [1.0, value]])
    }

    /// Control points, sorted by x.
    #[must_use]
    pub fn points(&self) -> &[[f32; 2]] {
        &self.points
    }

    /// Whether this is the identity.
    #[must_use]
    pub fn is_linear(&self) -> bool {
        self.points == [[0.0, 0.0], [1.0, 1.0]]
    }

    /// Value at `x` (clamped to 0–1).
    #[must_use]
    pub fn eval(&self, x: f32) -> f32 {
        let t = x.clamp(0.0, 1.0) * CURVE_STEPS as f32;
        let i = t as usize;
        if i >= CURVE_STEPS {
            return self.lut[CURVE_STEPS];
        }
        let f = t - i as f32;
        self.lut[i] + (self.lut[i + 1] - self.lut[i]) * f
    }
}

impl Default for Curve {
    fn default() -> Self {
        Self::linear()
    }
}

impl PartialEq for Curve {
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
    }
}

impl std::fmt::Debug for Curve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Curve").field(&self.points).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Curve;

    #[test]
    fn curve_endpoints_identity_and_clamp() {
        let linear = Curve::linear();
        assert!(linear.is_linear());
        for x in [0.0, 0.25, 0.5, 1.0] {
            assert!((linear.eval(x) - x).abs() < 1e-5);
        }
        assert!((linear.eval(2.0) - 1.0).abs() < 1e-6);
        // A soft curve stays monotonic and inside 0–1.
        let soft = Curve::new(&[[0.0, 0.0], [0.5, 0.2], [1.0, 1.0]]);
        let mut last = -1.0;
        for i in 0..=100 {
            let y = soft.eval(i as f32 / 100.0);
            assert!((0.0..=1.0).contains(&y));
            assert!(y >= last - 1e-4, "not monotonic at {i}");
            last = y;
        }
        assert!((soft.eval(0.5) - 0.2).abs() < 1e-3);
        // Flat outside the points; one point is not a curve.
        let mid = Curve::new(&[[0.2, 0.3], [0.8, 0.9]]);
        assert!((mid.eval(0.0) - 0.3).abs() < 1e-6);
        assert!((mid.eval(1.0) - 0.9).abs() < 1e-6);
        assert!(Curve::new(&[[0.5, 0.5]]).is_linear());
    }
}
