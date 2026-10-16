//! Portable Gregorian date identity and ordering, without session conversion.

/// Distinguish unproved representation from a provably invalid calendar value.
pub(crate) enum DateTextError {
    Syntax,
    Calendar,
}

/// Validate the fixed-width common Gregorian date subset (years 0001..9999).
/// Returning the original slice makes byte ordering a certified calendar order.
pub(crate) fn canonical_date_text(text: &str) -> Result<&str, DateTextError> {
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return Err(DateTextError::Syntax);
    }
    let year = text
        .get(..4)
        .ok_or(DateTextError::Syntax)?
        .parse::<u16>()
        .map_err(|_| DateTextError::Syntax)?;
    let month = text
        .get(5..7)
        .ok_or(DateTextError::Syntax)?
        .parse::<u8>()
        .map_err(|_| DateTextError::Syntax)?;
    let day = text
        .get(8..)
        .ok_or(DateTextError::Syntax)?
        .parse::<u8>()
        .map_err(|_| DateTextError::Syntax)?;
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return Err(DateTextError::Calendar),
    };
    if year != 0 && day != 0 && day <= max_day {
        Ok(text)
    } else {
        Err(DateTextError::Calendar)
    }
}
