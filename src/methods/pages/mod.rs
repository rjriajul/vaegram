mod dialog_pages;
mod offset_pages;

pub(crate) use dialog_pages::DialogPages;
pub(crate) use offset_pages::OffsetPages;

/// Converts a caller's count to the `i32` Telegram takes. Counts above `i32::MAX` saturate, since Telegram caps them
/// server-side anyway.
pub(crate) fn wire_limit(limit: usize) -> i32 {
    i32::try_from(limit).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::wire_limit;

    #[test]
    fn wire_limit_keeps_small_counts_and_saturates_large_ones() {
        assert_eq!(wire_limit(0), 0);
        assert_eq!(wire_limit(100), 100);
        assert_eq!(wire_limit(i32::MAX as usize), i32::MAX);
        assert_eq!(wire_limit(usize::MAX), i32::MAX);
    }
}
