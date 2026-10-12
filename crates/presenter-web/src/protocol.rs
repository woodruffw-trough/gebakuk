//! Pure validation shared by DOM events and inter-window messages.

use presenter_core::Layout;

pub(crate) fn bounded_integer(value: f64, maximum: u32) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= f64::from(maximum) && value.fract() == 0.0)
        .then_some(value as u32)
}

pub(crate) fn page_index(value: f64, page_count: usize) -> Option<usize> {
    let last = u32::try_from(page_count.checked_sub(1)?).ok()?;
    bounded_integer(value, last).map(|page| page as usize)
}

pub(crate) fn layout(value: &str) -> Option<Layout> {
    match value {
        "normal" => Some(Layout::Normal),
        "notes-right" => Some(Layout::NotesRight),
        _ => None,
    }
}

pub(crate) fn navigation_target(current: usize, page_count: usize, action: &str) -> Option<usize> {
    let last = page_count.checked_sub(1)?;
    let current = current.min(last);
    match action {
        "next" => Some(current.saturating_add(1).min(last)),
        "previous" => Some(current.saturating_sub(1)),
        "first" => Some(0),
        "last" => Some(last),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_identifiers_reject_fractions_and_out_of_range_numbers() {
        assert_eq!(bounded_integer(0.0, u32::MAX), Some(0));
        assert_eq!(
            bounded_integer(f64::from(u32::MAX), u32::MAX),
            Some(u32::MAX)
        );
        for value in [
            -1.0,
            0.5,
            f64::from(u32::MAX) + 1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(bounded_integer(value, u32::MAX), None, "accepted {value}");
        }
    }

    #[test]
    fn slide_indices_require_an_existing_slide() {
        assert_eq!(page_index(0.0, 0), None);
        assert_eq!(page_index(0.0, 1), Some(0));
        assert_eq!(page_index(2.0, 3), Some(2));
        for value in [-1.0, 1.5, 3.0, f64::NAN, f64::INFINITY] {
            assert_eq!(page_index(value, 3), None, "accepted {value}");
        }
    }

    #[test]
    fn layouts_accept_only_the_protocol_values() {
        assert!(matches!(layout("normal"), Some(Layout::Normal)));
        assert!(matches!(layout("notes-right"), Some(Layout::NotesRight)));
        for value in ["", "Normal", "notes-left", " notes-right", "notes-right\0"] {
            assert!(layout(value).is_none(), "accepted {value:?}");
        }
    }

    #[test]
    fn navigation_stays_inside_the_document() {
        assert_eq!(navigation_target(0, 3, "previous"), Some(0));
        assert_eq!(navigation_target(0, 3, "next"), Some(1));
        assert_eq!(navigation_target(2, 3, "next"), Some(2));
        assert_eq!(navigation_target(2, 3, "previous"), Some(1));
        assert_eq!(navigation_target(2, 3, "first"), Some(0));
        assert_eq!(navigation_target(0, 3, "last"), Some(2));
        assert_eq!(navigation_target(usize::MAX, 3, "next"), Some(2));
        assert_eq!(navigation_target(0, 3, "unknown"), None);
    }

    #[test]
    fn empty_and_single_slide_documents_handle_every_navigation_command() {
        for action in ["next", "previous", "first", "last"] {
            assert_eq!(navigation_target(0, 0, action), None);
            assert_eq!(navigation_target(0, 1, action), Some(0));
        }
    }
}
