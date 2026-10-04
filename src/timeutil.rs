/// UTC timestamp as unix milliseconds. Accepts `YYYY-MM-DDTHH:MM:SSZ` and a fractional suffix.
pub fn parse_unix_ms(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.len() < 20 || !text.is_char_boundary(19) {
        return None;
    }
    let year: i32 = text.get(0..4)?.parse().ok()?;
    let month: u32 = text.get(5..7)?.parse().ok()?;
    let day: u32 = text.get(8..10)?.parse().ok()?;
    let hour: u32 = text.get(11..13)?.parse().ok()?;
    let minute: u32 = text.get(14..16)?.parse().ok()?;
    let second: u32 = text.get(17..19)?.parse().ok()?;
    if text.as_bytes().get(4) != Some(&b'-')
        || text.as_bytes().get(7) != Some(&b'-')
        || text.as_bytes().get(10) != Some(&b'T')
        || text.as_bytes().get(13) != Some(&b':')
        || text.as_bytes().get(16) != Some(&b':')
    {
        return None;
    }
    let mut millis: i64 = 0;
    let rest = &text[19..];
    let rest = if let Some(stripped) = rest.strip_prefix('.') {
        let digits: String = stripped.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let padded = format!("{digits:0<3}");
        millis = padded.get(0..3)?.parse().ok()?;
        &stripped[digits.len()..]
    } else {
        rest
    };
    if rest != "Z" {
        return None;
    }
    if !(1..=12).contains(&month) || day == 0 || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400_000 + hour as i64 * 3_600_000 + minute as i64 * 60_000 + second as i64 * 1_000 + millis)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u32;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era as i64 * 146_097 + doe as i64 - 719_468)
}

#[cfg(test)]
mod tests {
    use super::parse_unix_ms;

    #[test]
    fn one_hour_between_fresh_and_the_fixture_clock() {
        let now = parse_unix_ms("2026-09-22T12:00:00.000Z").unwrap();
        let published = parse_unix_ms("2026-09-22T11:00:00.000Z").unwrap();
        assert_eq!((now - published) / 60_000, 60);
    }
}
