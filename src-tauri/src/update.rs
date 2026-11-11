//! Version comparison for the update check. Pure logic, unit-tested.

/// "v1.2.3", "1.2.3-beta" or "1.2" -> (major, minor, patch). Missing parts count as 0.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let trimmed = s.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    let patch = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    Some((major, minor, patch))
}

/// True when `latest` is a higher version than `current`. Anything unparsable is "not newer".
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_tag_shapes() {
        assert_eq!(parse_version("v1.0.1"), Some((1, 0, 1)));
        assert_eq!(parse_version("1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse_version("V2.3"), Some((2, 3, 0)));
        assert_eq!(parse_version("v1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("nightly"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn compares_numerically_not_alphabetically() {
        assert!(is_newer("v1.1.0", "1.0.1"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("v1.0.1", "1.1.0"));
        assert!(!is_newer("v1.1.0", "1.1.0"));
        assert!(!is_newer("garbage", "1.1.0"));
    }
}
