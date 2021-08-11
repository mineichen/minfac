use super::{error_pages::NotFoundHandler, *};
use anyhow::Result;
use futures::future::BoxFuture;
use hyper::{
    service::{make_service_fn, service_fn},
    Server,
};
use minfac::{AllRegistered, Registered, WeakServiceProvider};
use raf_hosted_service::HostedService;
use std::net::SocketAddr;

pub mod error_pages;

pub fn register_services(collection: &mut ServiceCollection) {
    collection
        .with::<(WeakServiceProvider, AllRegistered<ServiceProviderRoute>)>()
        .register(|(provider, routes)| {
            Box::new(HostedServer {
                provider,
                routes: routes.collect(),
            }) as Box<dyn HostedService>
        });
    error_pages::register_services(collection);
}

struct HostedServer {
    provider: WeakServiceProvider,
    routes: Vec<ServiceProviderRoute>,
}

type WebProviderRemainer = (Route, Arc<Request>);

impl HostedService for HostedServer {
    fn start(self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        let p = self.provider.clone();
        let mut web_collection = ServiceCollection::new();

        web_collection
            .with::<Registered<WebProviderRemainer>>()
            .register(|r| r.0);

        web_collection
            .with::<Registered<WebProviderRemainer>>()
            .register(|r| r.1);

        for route in self.routes.iter() {
            route.handler.register_dummy_dependency(&mut web_collection);
        }

        let factory = Arc::new(
            web_collection
                .with_parent(p)
                .build_factory::<WebProviderRemainer>()
                .unwrap(),
        );
        let cloned = self.routes.clone();

        async move {
            let mut router_builder = reset_recognizer::Router::build();

            println!("Add all routes: {}", cloned.len());
            for route in cloned {
                router_builder = router_builder.add(route.path.to_owned(), route);
            }

            let router = router_builder.finish()?;

            let sendable_router = Arc::new(router);

            let make_svc = make_service_fn(move |_conn| {
                let local_routes = sendable_router.clone();
                let local_factory = factory.clone();
                async {
                    Ok::<_, Infallible>(service_fn(move |req| {
                        dbg!(req.uri().path());
                        match local_routes.recognize(req.uri().path()) {
                            Ok(route) => {
                                println!("Request: {:?}", req.body());
                                let provider = local_factory
                                    .build((Route(Some(Arc::new(route.captures))), Arc::new(req)));
                                route.handler.call(provider)
                            }
                            _ => {
                                let provider = local_factory.build((Route(None), Arc::new(req)));
                                let handler =
                                    provider.resolve_unchecked::<Registered<NotFoundHandler>>();
                                handler.call(provider)
                            }
                        }
                    }))
                }
            });

            let server = Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000))).serve(make_svc);

            // Run this server for... forever!
            if let Err(e) = server.await {
                eprintln!("server error: {}", e);
            }
            Ok(())
        }
        .boxed()
    }
}
