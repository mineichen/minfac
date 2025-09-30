use core::cmp::Ord;

/// Returns the first position of the needle in the input if any
pub fn binary_search_first_by_key<T, U, TFn>(input: &[T], needle: &U, find: TFn) -> Option<usize>
where
    U: Ord,
    TFn: Fn(&T) -> &U,
{
    let x = input.partition_point(|x| find(x) < needle);
    let n = input.get(x)?;
    (find(n) == needle).then_some(x)
}

/// Returns the last position of the needle in the haystack or none, if not found
pub fn binary_search_last_by_key<T, U, TFn>(input: &[T], needle: &U, find: TFn) -> Option<usize>
where
    U: Ord,
    TFn: Fn(&T) -> &U,
{
    let x = input
        .partition_point(|x| find(x) <= needle)
        .checked_sub(1)?;
    let n = input.get(x)?;
    (find(n) == needle).then_some(x)
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
