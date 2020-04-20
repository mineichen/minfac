use super::{Container, Resolvable};
use core::{
    any::{Any},
    marker::PhantomData
};

pub struct ResolvableBuilder<TFn: FnOnce(Container<'_>)>(TFn);

impl<TFn: FnOnce(Container<'_>)> ResolvableBuilder<TFn> {    
    pub fn new(tfn: TFn) -> ResolvableBuilder<TFn> {
        ResolvableBuilder(tfn)
    }
    pub fn with_dependency<T: Resolvable>(self) -> TypedResolvableBuilder<impl FnOnce(Container<'_>), T> {
        TypedResolvableBuilder::<_, T> {
            next: self.0,
            dependency: PhantomData::<T>
        }
    }
    pub fn add<T: Any, TFactory: Fn(&dyn Fn(&T))>(self, factory: TFactory) -> ResolvableBuilder<impl FnOnce(Container<'_>)> {
        ResolvableBuilder(|container| {
            container.add::<(), _, _, _>(
                move|(), resolve| factory(resolve), 
                self.0
            )
        })
    }
    pub fn append_to(self, c: Container) {
        (self.0)(c);
    }
}

pub struct TypedResolvableBuilder<TFn: FnOnce(Container<'_>), T: Resolvable> {
    next: TFn,
    dependency: PhantomData<T>
}

impl<TFn: FnOnce(Container<'_>), TResolvable: Resolvable> TypedResolvableBuilder<TFn, TResolvable> {   
    pub fn add<T: Any, TFactory: Fn(&TResolvable::Result, &dyn Fn(&T))>(self, factory: TFactory) -> ResolvableBuilder<impl FnOnce(Container<'_>)> {
        ResolvableBuilder(|container| {
            container.add::<TResolvable, _, _, _>(
                factory, 
                self.next
            )
        })
    }
}