use anyhow::Result;
use futures::future::BoxFuture;

pub trait HostedService {
    fn start(self: Box<Self>) -> BoxFuture<'static, Result<()>>;
}
