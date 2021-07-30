use futures::{future::BoxFuture, FutureExt};
use ioc_rs::ServiceCollection;
use raf_hosted_service::TimeService;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

pub(crate) fn register_services(collection: &mut ServiceCollection) {
    collection.register(|| Arc::new(TokioTimeService) as Arc<dyn TimeService>);
}

struct TokioTimeService;
impl TimeService for TokioTimeService {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        sleep(duration).boxed()
    }
}
