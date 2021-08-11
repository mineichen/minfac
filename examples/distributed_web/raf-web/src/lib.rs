use anyhow::Result;
use bytes::Bytes;
use futures::{Future, FutureExt};
use ioc_rs::{Resolvable, ServiceBuilder, ServiceCollection, ServiceProvider};
use std::{borrow::Cow, convert::Infallible, pin::Pin, sync::Arc};

pub mod error_pages;

#[cfg(feature = "service")]
pub mod service;

pub trait ServiceCollectionWebExtensions {
    type Dependency: Send;

    fn register_web_handler<TPath, TFut>(
        &mut self,
        path: TPath,
        handler: fn(Self::Dependency) -> TFut,
    ) where
        TPath: Into<Cow<'static, str>>,
        TFut: Future<Output = HandlerFutureResult> + Send + 'static;
}

#[derive(Clone)]
pub struct Route(Option<Arc<reset_recognizer::Captures>>);
impl Route {
    pub fn get(&self, i: usize) -> Option<&str> {
        match &self.0 {
            Some(x) => x.get(i),
            None => None,
        }
    }
}

pub type Request = http::Request<hyper::Body>;
pub type Response = http::Response<hyper::Body>;
type HandlerResult = Pin<Box<dyn Future<Output = HandlerFutureResult> + Send>>;
type HandlerFutureResult = Result<Response>;

pub enum Body {
    Once(Option<Bytes>),
}

impl<'col, TDep> ServiceCollectionWebExtensions for ServiceBuilder<'col, TDep>
where
    TDep: Resolvable + Send,
    TDep::ItemPreChecked: Send,
{
    type Dependency = TDep::ItemPreChecked;

    fn register_web_handler<TPath, TFut>(
        &mut self,
        path: TPath,
        handler: fn(TDep::ItemPreChecked) -> TFut,
    ) where
        TPath: Into<Cow<'static, str>>,
        TFut: Future<Output = Result<Response>> + Send + 'static,
    {
        self.0.register_instance(ServiceProviderRoute::new(
            path.into(),
            ServiceProviderHandlerImpl::<TDep, TFut>::new_boxed(handler),
        ))
    }
}

struct ServiceProviderRoute {
    handler: Box<dyn ServiceProviderHandler>,
    path: Cow<'static, str>,
}

impl Clone for ServiceProviderRoute {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            handler: self.handler.clone_box(),
        }
    }
}

impl ServiceProviderRoute {
    fn new(path: Cow<'static, str>, handler: Box<dyn ServiceProviderHandler>) -> Self {
        Self { path, handler }
    }

    fn call(&self, provider: ServiceProvider) -> HandlerResult {
        self.handler.call(provider)
    }
}

pub trait ServiceProviderHandler: Send + Sync {
    fn clone_box(&self) -> Box<dyn ServiceProviderHandler>;
    fn call(&self, provider: ServiceProvider) -> HandlerResult;
    fn register_dummy_dependency(&self, col: &mut ServiceCollection);
}

impl<TFn, TFut> ServiceProviderHandler for TFn
where
    TFn: Fn() -> TFut + Send + Sync + Clone + 'static,
    TFut: Future<Output = HandlerFutureResult> + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn ServiceProviderHandler> {
        Box::new(self.clone())
    }

    fn call(&self, _provider: ServiceProvider) -> HandlerResult {
        (self)().boxed()
    }

    fn register_dummy_dependency(&self, _col: &mut ServiceCollection) {}
}

struct ServiceProviderHandlerImpl<TDep: Resolvable, TFut: Future<Output = Result<Response>> + Send>
{
    handler: fn(TDep::ItemPreChecked) -> TFut,
}

impl<TDep: Resolvable, TFut: Future<Output = Result<Response>> + Send>
    ServiceProviderHandlerImpl<TDep, TFut>
{
    fn new_boxed(handler: fn(TDep::ItemPreChecked) -> TFut) -> Box<Self> {
        Box::new(ServiceProviderHandlerImpl::<TDep, TFut> { handler })
    }
}

impl<TDep, TFut> ServiceProviderHandler for ServiceProviderHandlerImpl<TDep, TFut>
where
    TDep: Resolvable + Send,
    TDep::ItemPreChecked: Send,
    TFut: Future<Output = Result<Response>> + Send + 'static,
{
    fn call(&self, provider: ServiceProvider) -> HandlerResult {
        let handler = self.handler;
        async move { (handler)(provider.resolve_unchecked::<TDep>()).await }.boxed()
    }

    fn register_dummy_dependency(&self, col: &mut ServiceCollection) {
        col.with::<TDep>().register(|_| ());
    }

    fn clone_box(&self) -> Box<dyn ServiceProviderHandler> {
        Box::new(ServiceProviderHandlerImpl::<TDep, TFut> {
            handler: self.handler,
        })
    }
}
impl Clone for Box<dyn ServiceProviderHandler> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
