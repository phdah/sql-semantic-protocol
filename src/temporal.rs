//! Portable Gregorian date identity and ordering, without session conversion.

/// Validate the fixed-width common Gregorian date subset (years 0001..9999).
/// Returning the original slice makes byte ordering a certified calendar order.
pub(crate) fn canonical_date_text(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return None;
    }
    let year = text.get(..4)?.parse::<u16>().ok()?;
    let month = text.get(5..7)?.parse::<u8>().ok()?;
    let day = text.get(8..)?.parse::<u8>().ok()?;
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return None,
    };
    (year != 0 && day != 0 && day <= max_day).then_some(text)
}
