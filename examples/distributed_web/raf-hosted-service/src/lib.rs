use anyhow::Result;
use futures::future::BoxFuture;
use std::time::Duration;

pub trait HostedService {
    fn start(self: Box<Self>) -> BoxFuture<'static, Result<()>>;
}

pub trait TimeService: Send + Sync {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()>;
}
