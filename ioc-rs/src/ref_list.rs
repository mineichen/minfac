pub trait RefList<'a, T: 'a> {
    type Iterator: Iterator<Item=&'a T>;

    fn iter(&'a self) -> Self::Iterator;
    fn prepend(&'a self, value: T) -> Self;
}

pub enum OptionRefList<'a, T> {
    Some(T, &'a OptionRefList<'a, T>),
    None()
}



impl<'a, T> RefList<'a, T> for OptionRefList<'a, T> {
    //type ItemType = OptionRefList<'a, T>;
    type Iterator = RefListIterator<'a, T>;
    fn iter(&'a self) -> RefListIterator<'a, T> {
        RefListIterator(self)
    }
    fn prepend(&'a self, value: T) -> Self {
        OptionRefList::Some(value, self)
    }
}

pub struct RefListIterator<'a, T>(&'a OptionRefList<'a, T>);

impl<'a, T: 'a> Iterator for RefListIterator<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if let OptionRefList::Some(value, next) = self.0 {
            self.0 = next;
            return Some(value);
        }
        None
    }
}

impl<'a, T> Default for OptionRefList<'a, T> {
    fn default() -> Self { OptionRefList::<T>::None()}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_test() {
        let option_list = OptionRefList::<i32>::Some(10, &OptionRefList::<i32>::None());
        assert!(1 == option_list.iter().count());
    }

    #[test]
    fn list_add() {
        let mut option_list = OptionRefList::Some(10, &OptionRefList::None());
        let mut extended_list = option_list.prepend(10);
        assert_eq!(2, extended_list.iter().count());
    }
}