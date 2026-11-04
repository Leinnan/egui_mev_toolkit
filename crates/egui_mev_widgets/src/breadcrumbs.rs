//! Measuring breadcrumbs: which crumbs of a path fit a width. Drawing them is
//! left to the app, which knows what a crumb opens.

/// Which crumbs to draw.
#[derive(Debug, PartialEq, Eq)]
pub struct Fit {
    /// Indices of the crumbs drawn, in order.
    pub shown: Vec<usize>,
    /// Indices of the crumbs behind "…".
    pub hidden: Vec<usize>,
    /// The anchor shows only its icon.
    pub compact_anchor: bool,
}

/// Fits crumbs of `widths` into `available`. The current folder is kept longest,
/// then its nearest parents, then the anchor (first by dropping its name, then
/// entirely). `compact_anchor` is the anchor's width as an icon, `ellipsis` the
/// width of "…" and `sep` the width of one chevron.
#[must_use]
pub fn fit(widths: &[f32], compact_anchor: f32, ellipsis: f32, sep: f32, available: f32) -> Fit {
    let n = widths.len();
    let all = |compact| Fit {
        shown: (0..n).collect(),
        hidden: Vec::new(),
        compact_anchor: compact,
    };
    if n == 0 {
        return all(false);
    }
    let total = sep.mul_add((n - 1) as f32, widths.iter().sum::<f32>());
    if total <= available || n == 1 {
        return all(false);
    }
    let compact_total = total - widths[0] + compact_anchor.min(widths[0]);
    if compact_total <= available {
        return all(true);
    }
    let mut used = compact_anchor + sep + ellipsis + sep + widths[n - 1];
    if used > available {
        // Only the current folder: everything else goes behind "…".
        return Fit {
            shown: vec![n - 1],
            hidden: (0..n - 1).collect(),
            compact_anchor: false,
        };
    }
    let mut first = n - 1;
    while first > 1 && used + sep + widths[first - 1] <= available {
        first -= 1;
        used += sep + widths[first];
    }
    let mut shown = vec![0];
    shown.extend(first..n);
    Fit {
        shown,
        hidden: (1..first).collect(),
        compact_anchor: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_everything_when_it_fits() {
        let fit = fit(&[40.0, 60.0, 60.0], 28.0, 28.0, 40.0, 240.0);
        assert_eq!(fit.shown, [0, 1, 2]);
        assert_eq!(fit.hidden, Vec::<usize>::new());
        assert!(!fit.compact_anchor);
    }

    #[test]
    fn drops_the_anchor_name_before_any_folder() {
        // 100 + 60 + 60 + 2 * 40 = 300; with a 28px anchor: 228.
        let fit = fit(&[100.0, 60.0, 60.0], 28.0, 28.0, 40.0, 240.0);
        assert_eq!(fit.shown, [0, 1, 2]);
        assert!(fit.compact_anchor);
    }

    #[test]
    fn collapses_from_the_middle_keeping_the_nearest_parents() {
        let widths = [80.0, 60.0, 60.0, 60.0, 60.0, 60.0];
        // anchor 28 + 40 + "…" 28 + 40 + last 60 = 196; each parent adds 100.
        let fit = fit(&widths, 28.0, 28.0, 40.0, 380.0);
        assert_eq!(fit.shown, [0, 4, 5]);
        assert_eq!(fit.hidden, [1, 2, 3]);
        assert!(fit.compact_anchor);
    }

    #[test]
    fn keeps_only_the_current_folder_when_very_narrow() {
        let fit = fit(&[80.0, 60.0, 120.0], 28.0, 28.0, 40.0, 200.0);
        assert_eq!(fit.shown, [2]);
        assert_eq!(fit.hidden, [0, 1]);
    }

    #[test]
    fn no_crumbs_fit_any_width() {
        let fit = fit(&[], 28.0, 28.0, 40.0, 0.0);
        assert!(fit.shown.is_empty() && fit.hidden.is_empty());
    }

    #[test]
    fn one_crumb_is_kept_even_with_no_room() {
        let fit = fit(&[200.0], 28.0, 28.0, 40.0, 10.0);
        assert_eq!(fit.shown, [0]);
        assert!(!fit.compact_anchor);
    }

    #[test]
    fn the_current_folder_survives_every_narrow_width() {
        let widths = [80.0, 60.0, 60.0, 60.0, 120.0];
        let mut last_shown = usize::MAX;
        // Shrinking in steps: every crumb is either shown or hidden, the current
        // folder is always shown, and no width shows more than a wider one did.
        for available in (0..=600).rev().step_by(10) {
            let fit = fit(&widths, 28.0, 28.0, 40.0, available as f32);
            assert_eq!(fit.shown.last(), Some(&4), "at {available}");
            let mut all: Vec<usize> = fit.shown.iter().chain(&fit.hidden).copied().collect();
            all.sort_unstable();
            assert_eq!(all, [0, 1, 2, 3, 4], "at {available}");
            assert!(fit.shown.len() <= last_shown, "at {available}");
            last_shown = fit.shown.len();
        }
        assert_eq!(last_shown, 1);
    }
}
