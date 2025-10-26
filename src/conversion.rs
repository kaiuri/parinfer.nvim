use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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
        .unwrap_or(line.width_cjk())
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

type Diff = [usize; 4];
/// Converts two slices `a` and `b` to their differing ranges
/// given
/// - `a = [0,1,2,3]`
/// - `b = [3,2,1,0]`
///
/// then `diff = [0, 3, 0, 3]` where
/// - `a[diff[0]..=diff[1]]` is which slice of a is different from b
/// - `b[diff[2]..=diff[3]]` is which slice of b is different from a
pub fn diff_slice<T: std::cmp::Eq>(a: &[T], b: &[T]) -> Option<Diff> {
    if a.is_empty() && b.is_empty() {
        // they're equally empty
        return None;
    } else if a.is_empty() || b.is_empty() {
        // one is empty, other is not
        return Some([0, a.len().saturating_sub(1), 0, b.len().saturating_sub(1)]);
    }
    let mut it = a.iter().enumerate().zip(b.iter().enumerate());
    let stop = it.rfind(|(a, b)| a != b)?;
    let start = it.find(|(a, b)| a != b).unwrap_or(stop);// we've exausted the iterator, so only a single element is different
    Some([start.0.0, stop.0.0, start.1.0, stop.1.0])
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
    fn diff_slice_chunks(a: Vec<u8>, b: Vec<u8>) -> bool {
        if let Some([start_a, end_a, start_b, end_b]) = diff_slice(&a, &b) {
            // if they're different
            // at least one isn't empty
            if a.is_empty() && b.is_empty() {
                return false;
            }
            // everything left of start is the same
            let left = a[0..start_a] == b[0..start_b];
            let right_start_a = std::cmp::min(end_a, a.len());
            let right_start_b = std::cmp::min(end_b, b.len());
            // and everything right of end is the same
            let right = a[right_start_a..end_a] == b[right_start_b..end_b];
            left && right
        } else {
            // if there's no difference, they're equal
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
    fn test_diff_slice() {
        let samples = [
            (vec![1, 2, 3], vec![3, 2, 1], Some([0, 2, 0, 2])),
            (vec![1, 2, 3, 4], vec![1, 2, 4, 3], Some([2, 3, 2, 3])),
            (vec![1, 2, 3, 4], vec![1, 2, 4, 4], Some([2, 2, 2, 2])),
            (vec![], vec![1, 2, 4, 4], Some([0, 0, 0, 3])),
            (vec![1, 2, 4, 4], vec![], Some([0, 3, 0, 0])),
        ];
        for (a, b, expected_diff) in samples {
            let diff = diff_slice(&a, &b);
            assert_eq!(
                diff, expected_diff,
                "{a:?} {b:?} {diff:?} {expected_diff:?}",
            );
        }
    }
}
