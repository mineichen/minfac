use crate::{HandlerResult, ServiceProviderHandler};
use minfac::ServiceProvider;

pub struct NotFoundHandler(Box<dyn super::ServiceProviderHandler>);

impl NotFoundHandler {
    pub fn new(a: impl ServiceProviderHandler + 'static) -> Self {
        Self(Box::new(a))
    }
    pub fn call(&self, provider: ServiceProvider) -> HandlerResult {
        self.0.call(provider)
    }
}
