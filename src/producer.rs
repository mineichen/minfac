use core::Any;

trait Producer {
    /// This type is usually casted from T -> () when stored in ServiceCollection.register_*
    /// and () -> T on ServiceProvider.get<T>() 
    type Result: for<'a> FamilyLt<'a>;

    fn resolve<'a>(&self, provider: &'a ServiceProvider) -> <Self::Result as FamilyLt<'a>>::Out;
}

struct TransientProducer<T: Any> {
    producer_function: fn() -> T
}

impl<T: Any> Producer for TransientProducer<T> {
    type Result = crate::family_lifetime::IdFamily<T>;

    fn resolve<'a>(&self, provider: &'a ServiceProvider) -> <Self::Result as FamilyLt<'a>>::Out {
        (self.producer_function)()
    }
}