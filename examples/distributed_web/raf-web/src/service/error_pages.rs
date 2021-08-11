use crate::{error_pages::NotFoundHandler, HandlerFutureResult};
use http::Response;
use minfac::ServiceCollection;

pub fn register_services(collection: &mut ServiceCollection) {
    collection.register(|| NotFoundHandler::new(not_found));
}

async fn not_found() -> HandlerFutureResult {
    Response::builder()
        .status(404)
        .body("404 - Not Found".into())
        .map_err(|_| unreachable!("foo")) // Todo: Remove when HandlersMayReturnError
}
