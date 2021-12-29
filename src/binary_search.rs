use core::cmp::Ord;

/// Returns the first position of the needle in the input if any
/// This function is optimized for searching input with multiple occurences of needle
///
/// Modified version with positive number index
/// https://www.geeksforgeeks.org/find-first-and-last-positions-of-an-element-in-a-sorted-array/
///
pub fn binary_search_first_by_key<T, U, TFn>(input: &[T], needle: &U, find: TFn) -> Option<usize>
where
    U: Ord,
    TFn: Fn(&T) -> &U,
{
    let mut result_idx = None;
    let mut front_plus_1 = 1;
    let mut end_plus_1 = input.len();
    while front_plus_1 <= end_plus_1 {
        let mid = (front_plus_1 + end_plus_1) / 2 - 1;
        let value = unsafe { input.get_unchecked(mid) };
        let sort_key = (find)(value);
        if U::lt(sort_key, needle) {
            front_plus_1 = mid + 2;
        } else {
            if U::eq(sort_key, needle) {
                result_idx = Some(mid);
            }
            end_plus_1 = mid;
        }
    }
    result_idx
}

/// Returns the last position of the needle in the haystack or none, if not found
/// This function is optimized for searching input with one or just a few occurences of needle
pub fn binary_search_last_by_key<T, U, TFn>(input: &[T], needle: &U, find: TFn) -> Option<usize>
where
    U: Ord,
    TFn: Fn(&T) -> &U,
{
    let mut front_plus_1 = 1;
    let mut end_plus_1 = input.len();
    while front_plus_1 <= end_plus_1 {
        let mid = (front_plus_1 + end_plus_1) / 2 - 1;
        let value = unsafe { input.get_unchecked(mid) };
        let sort_key = (find)(value);
        if U::gt(sort_key, needle) {
            end_plus_1 = mid;
        } else {
            if U::eq(sort_key, needle)
                && (mid + 1 == input.len()
                    || U::ne((find)(unsafe { input.get_unchecked(mid + 1) }), needle))
            {
                return Some(mid);
            }
            front_plus_1 = mid + 2;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_search_by_first_key() {
        test_first_or_last(|i, e| binary_search_first_by_key(i, e, |extr| extr));

        assert_eq!(Some(0), binary_search_first_by_key(&[0, 0, 0], &0, |a| a));
        assert_eq!(Some(1), binary_search_first_by_key(&[0, 1, 1], &1, |a| a));
    }

    #[test]
    fn test_binary_search_by_last_key() {
        test_first_or_last(|i, e| binary_search_last_by_key(i, e, |extr| extr));
        assert_eq!(Some(2), binary_search_last_by_key(&[0, 0, 0], &0, |a| a));
        assert_eq!(Some(2), binary_search_last_by_key(&[0, 1, 1], &1, |a| a));
        assert_eq!(Some(1), binary_search_last_by_key(&[0, 0, 1], &0, |a| a));
    }

    fn test_first_or_last<'a>(i: impl Fn(&[usize], &usize) -> Option<usize>) {
        assert_eq!(Some(0), (i)(&[0, 1, 2], &0));
        assert_eq!(Some(1), (i)(&[0, 1, 2], &1));
        assert_eq!(Some(2), (i)(&[0, 1, 2], &2));
        assert_eq!(Some(0), (i)(&[0], &0));
        assert_eq!(None, (i)(&[0, 1, 2], &3));
        assert_eq!(None, (i)(&[], &3));
    }
}
