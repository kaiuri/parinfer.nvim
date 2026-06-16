use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Converts a byte column 0-indexed to its visual column counterpart.
/// Will look for the first visual column whose byte column is either exactly equal to itself or greater than it.
/// Otherwise, byte column based movement would sometimes not move the cursor at all.
///
/// ```rust
/// assert_eq!(bytepos_to_charpos( "😀 ok!", 0), 0);
/// assert_eq!(bytepos_to_charpos( "😀 ok!", 1), 4);
/// ```
pub fn bytepos_to_charpos(line: &str, bytecol: usize) -> usize {
    line.grapheme_indices(true)
        .position(|(i, _)| i >= bytecol)
        .unwrap_or_else(|| line.grapheme_indices(true).count())
}
/// Converts a visual column 0-indexed to its byte column counterpart.
///
/// ```rust
/// assert_eq!(charpos_to_bytepos( "😀 ok!", 0), 0);
/// assert_eq!(charpos_to_bytepos( "😀 ok!", 1), 4);
/// ```
pub fn charpos_to_bytepos(line: &str, charcol: usize) -> usize {
    line.grapheme_indices(true)
        .nth(charcol)
        .map_or(line.len(), |(i, _)| i)
}

/// Identifies the smallest range in `a` and `b` that differs,
/// performing a parallel index-by-index comparison.
pub fn diff_ranges<T: std::cmp::Eq>(a: &[T], b: &[T]) -> Option<(Range<usize>, Range<usize>)> {
    let max_len = a.len().max(b.len());

    // Find the first index where the elements don't match (or one is missing)
    let start = (0..max_len).find(|&i| a.get(i) != b.get(i))?;

    // Find the last index where they don't match
    let stop = (0..max_len)
        .rfind(|&i| a.get(i) != b.get(i))
        .unwrap_or(start);

    // Calculate exclusive bounds, capping at the actual length of each slice
    let end_a = (stop + 1).min(a.len());
    let end_b = (stop + 1).min(b.len());

    let start_a = start.min(a.len());
    let start_b = start.min(b.len());

    Some((start_a..end_a, start_b..end_b))
}

#[cfg(test)]
mod tests {
    use super::*;

    // tests that if bytepos is at a char boundary ∀ bytepos , charpos_to_bytepos(bytepos_to_charpos(.., bytepos)) == bytepos
    #[quickcheck_macros::quickcheck]
    fn bytepos_charpos_idepontency(line: String, bytepos: usize) -> bool {
        if line.is_char_boundary(bytepos) {
            charpos_to_bytepos(&line, bytepos_to_charpos(&line, bytepos)) == bytepos
        } else {
            bytepos_to_charpos(
                &line,
                charpos_to_bytepos(&line, bytepos_to_charpos(&line, bytepos)),
            ) == bytepos_to_charpos(&line, bytepos)
        }
    }

    #[quickcheck_macros::quickcheck]
    fn diff_range_works(a: Vec<u8>, b: Vec<u8>) -> bool {
        if let Some((range_a, range_b)) = diff_ranges(&a, &b) {
            // A difference was found, so they cannot be identical empty slices
            if a.is_empty() && b.is_empty() {
                return false;
            }

            let left = a[..range_a.start] == b[..range_b.start];

            // 2. Everything strictly right of the difference must be identical
            let right = a[range_a.end..] == b[range_b.end..];

            // 3. (Optional but good) The differing ranges themselves shouldn't be identical
            // Unless one is empty and the other isn't (an insertion/deletion)
            let middle_differs = a[range_a.clone()] != b[range_b.clone()];

            left && right && middle_differs
        } else {
            // If there's no difference, the slices must be exactly equal
            a == b
        }
    }

    #[test]
    fn test_bytepos_to_charpos() {
        assert_eq!(bytepos_to_charpos("abc", 0), 0);
        assert_eq!(bytepos_to_charpos("abc", 1), 1);
        assert_eq!(bytepos_to_charpos("abc", 2), 2);
        assert_eq!(bytepos_to_charpos("åbc", 0), 0);
        assert_eq!(bytepos_to_charpos("åbc", 1), 1);
        assert_eq!(bytepos_to_charpos("åbc", 2), 1);
        assert_eq!(bytepos_to_charpos("åbc", 3), 2);
        assert_eq!(bytepos_to_charpos("ｗｏa", 0), 0);
        assert_eq!(bytepos_to_charpos("ｗｏa", 1), 1);
        assert_eq!(bytepos_to_charpos("ｗｏa", 2), 1);
        assert_eq!(bytepos_to_charpos("ｗｏa", 3), 1);
        assert_eq!(bytepos_to_charpos("ｗｏa", 4), 2);
        assert_eq!(bytepos_to_charpos("ｗｏa", 5), 2);
        assert_eq!(bytepos_to_charpos("ｗｏa", 6), 2);
    }

    #[test]
    fn test_charpos_to_bytepos() {
        assert_eq!(charpos_to_bytepos("", 0), 0);
        assert_eq!(charpos_to_bytepos("", 1), 0);
        assert_eq!(charpos_to_bytepos("abc", 0), 0);
        assert_eq!(charpos_to_bytepos("abc", 1), 1);
        assert_eq!(charpos_to_bytepos("abc", 2), 2);
        assert_eq!(charpos_to_bytepos("åbc", 0), 0);
        assert_eq!(charpos_to_bytepos("åbc", 1), 2);
        assert_eq!(charpos_to_bytepos("åbc", 2), 3);
        assert_eq!(charpos_to_bytepos("ｗｏa", 0), 0);
        assert_eq!(charpos_to_bytepos("ｗｏa", 1), 3);
        assert_eq!(charpos_to_bytepos("ｗｏa", 1), 3);
        assert_eq!(charpos_to_bytepos("ｗｏa", 1), 3);
        assert_eq!(charpos_to_bytepos("ｗｏa", 2), 6);
    }

    #[test]
    fn test_diff_ranges() {
        let samples = [
            (vec![1, 2, 3], vec![3, 2, 1], Some((0..3, 0..3))),
            (vec![1, 2, 3, 4], vec![1, 2, 4, 3], Some((2..4, 2..4))),
            (vec![1, 2, 3, 4], vec![1, 2, 4, 4], Some((2..3, 2..3))),
            (vec![], vec![1, 2, 4, 4], Some((0..0, 0..4))),
            (vec![1, 2, 4, 4], vec![], Some((0..4, 0..0))),
            (vec![1, 2], vec![1, 2, 3], Some((2..2, 2..3))),
        ];
        for (a, b, expected_diff) in samples {
            let diff = diff_ranges(&a, &b);
            assert_eq!(
                diff, expected_diff,
                "{a:?} {b:?} {diff:?} {expected_diff:?}",
            );
        }
    }
}
