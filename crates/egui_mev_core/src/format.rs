//! Human-readable sizes.

const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

fn scaled(bytes: u64) -> (f64, &'static str) {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    (value, UNITS[unit])
}

/// `"512 B"`, `"1.50 GB"`, `"25.6 MB"`, `"256 MB"`: three significant digits.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    let (value, unit) = scaled(bytes);
    // Bytes are whole: "0 B", not "0.00 B".
    if bytes < 1024 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {unit}")
    } else if value >= 10.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.2} {unit}")
    }
}

/// `"512B"`, `"1.5G"`, `"256M"`, for tight columns.
#[must_use]
pub fn format_bytes_compact(bytes: u64) -> String {
    let (value, unit) = scaled(bytes);
    let short = &unit[..1];
    if bytes < 1024 {
        format!("{bytes}B")
    } else if value >= 100.0 {
        format!("{value:.0}{short}")
    } else {
        format!("{value:.1}{short}")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_bytes_compact};

    #[test]
    fn whole_bytes_have_no_decimals() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes_compact(512), "512B");
    }

    #[test]
    fn larger_sizes_keep_three_significant_digits() {
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1_572_864), "1.50 MB");
        assert_eq!(format_bytes(26_843_546), "25.6 MB");
        assert_eq!(format_bytes(268_435_456), "256 MB");
        assert_eq!(format_bytes_compact(1_610_612_736), "1.5G");
    }

    #[test]
    fn the_largest_unit_does_not_overflow_the_table() {
        assert_eq!(format_bytes(u64::MAX), "16384 PB");
    }
}
