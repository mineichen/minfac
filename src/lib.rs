use {
    core::{
        marker::PhantomData,
        any::{Any, TypeId}
    }
};

mod binary_search;

// The family trait for type constructors that have one input lifetime.
pub trait FamilyLt<'a> {
    type Out;
}

#[derive(Debug)]
pub struct IdFamily<T: Any>(PhantomData<T>);
impl<'a, T: Any> FamilyLt<'a> for IdFamily<T> {
    type Out = T;
}

#[derive(Debug)]
pub struct RefFamily<T: Any>(PhantomData<T>);
impl<'a, T: 'a + Any> FamilyLt<'a> for RefFamily<T> {
    type Out = &'a T;
}

impl<'a, T: FamilyLt<'a>> FamilyLt<'a> for Option<T> {
    type Out = Option<T::Out>;
}

impl<'a, T0: FamilyLt<'a>, T1: FamilyLt<'a>> FamilyLt<'a> for (T0, T1) {
    type Out = (
        T0::Out,
        T1::Out
    );
}
impl<'a, T0: FamilyLt<'a>, T1: FamilyLt<'a>, T2: FamilyLt<'a>> FamilyLt<'a> for (T0, T1, T2) {
    type Out = (
        T0::Out,
        T1::Out,
        T2::Out
    );
}
impl<'a, T0: FamilyLt<'a>, T1: FamilyLt<'a>, T2: FamilyLt<'a>, T3: FamilyLt<'a>> FamilyLt<'a> for (T0, T1, T2, T3) {
    type Out = (
        T0::Out,
        T1::Out,
        T2::Out,
        T3::Out
    );
}

pub trait Resolvable: Any {
    type Item: for<'a> FamilyLt<'a>;
    type ItemPreChecked: for<'a> FamilyLt<'a>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out;
    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out;
}

impl Resolvable for () {
    type Item = IdFamily<()>;
    type ItemPreChecked = IdFamily<()>;

    fn resolve<'s>(_: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        ()
    }
    fn resolve_prechecked<'s>(_: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        ()
    }
}

impl<T0: Resolvable, T1: Resolvable> Resolvable for (T0, T1) {
    type Item = (T0::Item, T1::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked);
  
    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        (T0::resolve(container), T1::resolve(container))
    }
  
    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        (T0::resolve_prechecked(container), T1::resolve_prechecked(container))
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable> Resolvable for (T0, T1, T2) {
    type Item = (T0::Item, T1::Item, T2::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked, T2::ItemPreChecked);
  
    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        (T0::resolve(container), T1::resolve(container), T2::resolve(container))
    }
    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        (T0::resolve_prechecked(container), T1::resolve_prechecked(container), T2::resolve_prechecked(container))
    }
}
impl<T0: Resolvable, T1: Resolvable, T2: Resolvable, T3: Resolvable> Resolvable for (T0, T1, T2, T3) {
    type Item = (T0::Item, T1::Item, T2::Item, T3::Item);
    type ItemPreChecked = (T0::ItemPreChecked, T1::ItemPreChecked, T2::ItemPreChecked, T3::ItemPreChecked);

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        (
            T0::resolve(container), 
            T1::resolve(container),
            T2::resolve(container),
            T3::resolve(container)
        )
    }
    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        (
            T0::resolve_prechecked(container), 
            T1::resolve_prechecked(container),
            T2::resolve_prechecked(container),
            T3::resolve_prechecked(container)
        )
    }
}

impl Resolvable for ServiceProvider {
    type Item = RefFamily<ServiceProvider>;
    type ItemPreChecked  = RefFamily<ServiceProvider>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        container
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        Self::resolve(container)
    }
}
pub struct Singleton<T: Any>(PhantomData<T>);
impl<T: Any> Resolvable for Singleton<T> {
    type Item = Option<RefFamily<T>>;
    type ItemPreChecked = RefFamily<T>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        binary_search::binary_search_by_last_key(&container.producers, &TypeId::of::<Self>(), |(id, _)| id)
            .map(|f| {  
                unsafe { resolve_unchecked::<Self>(container, f) }
            })
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        Self::resolve(container).unwrap()
    }
}

unsafe fn resolve_unchecked<'a, T: Resolvable>(container: &'a ServiceProvider, pos: usize) -> <T::ItemPreChecked as FamilyLt<'a>>::Out {
    ({
        let func_ptr = container.producers.get_unchecked(pos).1 as *const dyn Fn(&ServiceProvider) -> <T::ItemPreChecked as FamilyLt<'a>>::Out;
        &* func_ptr
    })(&container)
}

pub struct ServiceIterator<'a, T> {
    next_pos: Option<usize>,
    provider: &'a ServiceProvider, 
    item_type: PhantomData<T>
}

impl<'a, T: Resolvable> std::iter::Iterator for ServiceIterator<'a, T> {
    type Item = <T::ItemPreChecked as FamilyLt<'a>>::Out;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_pos.map(|i| {
            self.next_pos = if let Some(next) = self.provider.producers.get(i + 1) {
                if next.0 == TypeId::of::<T>() { 
                    Some(i + 1) 
                } else {
                    None
                }
            } else {
                None
            };
            
            unsafe { resolve_unchecked::<T>(self.provider, i) }
        })
    }

    fn last(self) -> Option<Self::Item> where Self: Sized {
        self.next_pos.map(|i| {
            // If has_next, last must exist
            let pos = binary_search::binary_search_by_last_key(&self.provider.producers[i..], &TypeId::of::<T>(), |(id, _)| id).unwrap();
            unsafe { resolve_unchecked::<T>(self.provider, pos)}            
        }) 
    }
    fn count(self) -> usize where Self: Sized {
        self.next_pos.map(|i| {
            let pos = binary_search::binary_search_by_last_key(&self.provider.producers[i..], &TypeId::of::<T>(), |(id, _)| id).unwrap();
            pos - i         
        }).unwrap_or(0)
    }
}
pub struct ServiceIteratorFamily<T>(PhantomData<T>);

impl<'a, T: Resolvable> FamilyLt<'a> for ServiceIteratorFamily<T> {
    type Out = ServiceIterator<'a, T>;
}

pub struct TransientServices<T: Any>(PhantomData<T>);
impl<T: Any> Resolvable for TransientServices<T> {
    type Item = ServiceIteratorFamily<Transient<T>>;
    type ItemPreChecked = ServiceIteratorFamily<Transient<T>>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        let next_pos = binary_search::binary_search_by_first_key(&container.producers, &TypeId::of::<Transient<T>>(), |(id, _)| id);
        ServiceIterator { provider: &container, item_type: PhantomData, next_pos }
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        Self::resolve(container)
    }
}

pub struct Transient<T: Any>(PhantomData<T>);
impl<T: Any> Resolvable for Transient<T> {
    type Item = Option<IdFamily<T>>;
    type ItemPreChecked = IdFamily<T>;

    fn resolve<'s>(container: &'s ServiceProvider) -> <Self::Item as FamilyLt<'s>>::Out {
        binary_search::binary_search_by_last_key(&container.producers, &TypeId::of::<Self>(), |(id, _)| id)
            .map(|f| {    
                unsafe { resolve_unchecked::<Self>(container, f) }
            })
    }

    fn resolve_prechecked<'s>(container: &'s ServiceProvider) -> <Self::ItemPreChecked as FamilyLt<'s>>::Out {
        Self::resolve(container).unwrap()
    }
}


pub struct ServiceCollection {
    producers: Vec<(TypeId, *const dyn Fn())>,
}

impl ServiceCollection {
    pub fn new() -> Self {
        Self {
            producers: Vec::new()
        }
    }
} 

impl ServiceCollection {
    pub fn register_transient<'s, 'a: 's, TDependency: Resolvable, T: Any>(&'s mut self, creator: fn(<TDependency::ItemPreChecked as FamilyLt<'a>>::Out) -> T) {
        let func : Box<dyn Fn(&'a ServiceProvider) -> T> = Box::new(move |container: &'a ServiceProvider| {
            let arg = TDependency::resolve_prechecked(container);
            creator(arg)
        });
        
        self.producers.push((
            TypeId::of::<Transient<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
    pub fn register_singleton<'s, 'a: 's, TDependency: Resolvable, T: Any + Sync>(&'s mut self, creator: fn(<TDependency::ItemPreChecked as FamilyLt<'a>>::Out) -> T) {
        let cell = once_cell::sync::OnceCell::new();
        let func : Box<dyn Fn(&'a ServiceProvider) -> &T> = Box::new(move |container: &'a ServiceProvider| { 
            unsafe { 
                // Deref is valid because container cannot delete any producers
                // Unless destroying itself
                &*(cell.get_or_init(|| {
                    let arg = TDependency::resolve_prechecked(container);
                    creator(arg)
                }) as *const T)
            }
        });
        
        self.producers.push((
            TypeId::of::<Singleton<T>>(), 
            Box::into_raw(func) as *const dyn Fn()
        ));
    }
    pub fn build(mut self) -> ServiceProvider {
        self.producers.sort_by_key(|(id,_)| *id);
        ServiceProvider { producers: self.producers}
    }
}

pub struct ServiceProvider {
    /// Mustn't be changed because `resolve_unchecked` relies on it.
    producers: Vec<(TypeId, *const dyn Fn())>
}

impl ServiceProvider {
    pub fn get<'s, T: Resolvable>(&'s self) -> <T::Item as FamilyLt<'s>>::Out {
        T::resolve(self)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn resolve_last_transient() {
        let mut container = ServiceCollection::new();
        container.register_transient::<(), _>(|_| 0);
        container.register_transient::<(), _>(|_| 5);
        container.register_transient::<(), _>(|_| 1);
        container.register_transient::<(), _>(|_| 2);
        let provider = container.build();
        assert_eq!(
            2, 
            Transient::<i32>::resolve(&provider).unwrap()
        );
    }

    #[test]
    fn resolve_last_singleton() {
        let mut container = ServiceCollection::new();
        container.register_singleton::<(), _>(|_| 0);
        container.register_singleton::<(), _>(|_| 1);
        container.register_singleton::<(), _>(|_| 2);
        let provider = container.build();
        assert_eq!(
            2, 
            *Singleton::<i32>::resolve(&provider).unwrap()
        );
    }

    #[test]
    fn resolve_transient_services() {
        let mut container = ServiceCollection::new();
        container.register_transient::<(), _>(|_| 0);
        container.register_transient::<(), _>(|_| 5);
        container.register_transient::<(), _>(|_| 2);
        let provider = container.build();
        let mut iter = TransientServices::<i32>::resolve(&provider);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(5), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());        
    }

    #[test]
    fn resolve_test() {
        let mut container = ServiceCollection::new();
        container.register_transient::<(), _>(|_| 42);
        container.register_singleton::<ServiceProvider,_>(|_| 42);
        let provider = container.build();
        assert_eq!(
            Transient::<i32>::resolve(&provider).unwrap(), 
            Singleton::<i32>::resolve(&provider).map(|f| *f).unwrap()
        );
    }

    #[test]
    fn get_registered_dynamic_id() {
        let mut container = ServiceCollection::new();
        container.register_transient::<(),_>(|_| 42);
        assert_eq!(Some(42i32), container.build().get::<Transient<i32>>());
    }
    #[test]
    fn get_registered_dynamic_ref() {
        let mut container = ServiceCollection::new();
        container.register_singleton::<(), i32>(|_| 42);
        assert_eq!(Some(&42i32), container.build().get::<Singleton<i32>>());
    }

    #[test]
    fn tuple_dependency_resolves_to_prechecked_type() {
        let mut container = ServiceCollection::new();
        container.register_transient::<(), _>(|_| 64i64);
        container.register_singleton::<(Transient<i64>, Transient<i64>), _>(|(a, b)| {
            assert_eq!(64, a);
            assert_eq!(64, b);
            42
        });
        assert_eq!(Some(&42i32), container.build().get::<Singleton<i32>>());
    }

    #[test]
    fn get_unkown_returns_none() {
        let container = ServiceCollection::new();
        assert_eq!(None, container.build().get::<Transient<i32>>());
    }

    #[test]
    fn resolve_tuple_2() {
        let mut container = ServiceCollection::new();
        container.register_transient::<(), i32>(|_| 32);
        container.register_singleton::<(), i64>(|_| 64);
        assert_eq!((Some(32), Some(&64)), container.build().get::<(Transient<i32>, Singleton<i64>)>());
    }

    #[test]
    fn register_struct_as_dynamic() {
        let mut container = ServiceCollection::new();   
        container.register_singleton::<(), _>(|_| 42i32);
        container.register_singleton::<Singleton<i32>, _>(|i| ServiceImpl(i));
        container.register_transient::<Singleton<ServiceImpl>, _>(|c| c as &dyn Service);
        let service = container.build()
            .get::<Transient<&dyn Service>>()
            .expect("Expected to get a service");
        assert_eq!(42, service.get_value());
    }

    trait Service {
        fn get_value(&self) -> i32;
    }

    struct ServiceImpl<'a>(&'a i32);
    impl<'a> Service for ServiceImpl<'a> {
        fn get_value(&self) -> i32 {
            *self.0
        }
    }
}