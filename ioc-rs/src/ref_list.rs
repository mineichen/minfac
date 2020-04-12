pub enum RefList<'a, T> {
    Some(T, &'a RefList<'a, T>),
    None()
}

impl<'a, T> RefList<'a, T> {
    pub fn iter(&'a self) -> RefListIterator<'a, T> {
        RefListIterator(self)
    }
}

pub struct RefListIterator<'a, T>(&'a RefList<'a, T>);

impl<'a, T: 'a> Iterator for RefListIterator<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if let RefList::Some(value, next) = self.0 {
            self.0 = next;
            return Some(value);
        }
        None
    }
}

impl<'a, T> Default for RefList<'a, T> {
    fn default() -> Self { RefList::<T>::None()}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_test() {
        let option_list = RefList::<i32>::Some(10, &RefList::<i32>::None());

        assert!(1 == option_list.iter().count());
    }
}