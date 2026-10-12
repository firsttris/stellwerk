//! Syncthing's time tags `YYYYMMDD-HHMMSS`, in conflict copies and version names.

use super::{split_ext, split_path};

/// `YYYYMMDD-HHMMSS` as seconds since the epoch, read as UTC.
pub fn parse_tag(tag: &str) -> Option<i64> {
    let digits = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse::<i64>().ok())?
    };
    if tag.len() != 15 || tag.as_bytes()[8] != b'-' {
        return None;
    }
    let (year, month, day) = (
        digits(&tag[0..4])?,
        digits(&tag[4..6])?,
        digits(&tag[6..8])?,
    );
    let (hour, minute, second) = (
        digits(&tag[9..11])?,
        digits(&tag[11..13])?,
        digits(&tag[13..15])?,
    );
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Seconds since the epoch as `YYYYMMDD-HHMMSS` (UTC), Syncthing's version tag.
pub fn format_tag(seconds: i64) -> String {
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// `dir/name.ext` → `dir/name~tag.ext`, as Syncthing tags versions.
pub fn tag_filename(path: &str, tag: &str) -> String {
    let (dir, file) = split_path(path);
    let (stem, ext) = split_ext(file);
    format!("{dir}{stem}~{tag}{ext}")
}

// Howard Hinnant's algorithms for the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip_and_match_known_dates() {
        assert_eq!(parse_tag("19700101-000000"), Some(0));
        assert_eq!(parse_tag("20000301-000000"), Some(951_868_800));
        for seconds in [0, 951_868_800, 1_791_798_071] {
            assert_eq!(parse_tag(&format_tag(seconds)), Some(seconds));
        }
        assert_eq!(parse_tag("20261312-000000"), None);
        assert_eq!(
            tag_filename("a/b/config.ts", "20261012-103112"),
            "a/b/config~20261012-103112.ts"
        );
        assert_eq!(
            tag_filename(".bashrc", "20261012-103112"),
            ".bashrc~20261012-103112"
        );
    }
}
