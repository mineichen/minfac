use crate::repository::TodoRepository;
use anyhow::Result;
use futures::TryStreamExt;
use minfac::{Registered, ServiceCollection};
use raf_web::{Response, Route, ServiceCollectionWebExtensions};

pub(crate) fn register_services(collection: &mut ServiceCollection) {
    println!("Register TodoHandler");
    collection
        .with::<Registered<Box<dyn TodoRepository>>>()
        .register_web_handler("^/todo(/)?$", list);
    collection
        .with::<Registered<Route>>()
        .register_web_handler("^/param/(.*)?$", first_param);
}

async fn list(mut repo: Box<dyn TodoRepository>) -> Result<Response> {
    let all = repo.get_all().await?;
    Ok(Response::new(
        format!("Values: {:?}", all.try_collect::<Vec<_>>().await?).into(),
    ))
}

async fn first_param(route: Route) -> Result<Response> {
    Ok(Response::new(
        format!("Has Value: {:?}", route.get(1)).into(),
    ))
}
