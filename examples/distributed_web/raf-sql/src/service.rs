use crate::{
    DynConnectionWrapper, DynamicDatabaseExecutorPromise, FetchManyResult, SqliteConfiguration,
    SqliteExecutor,
};
use futures::{future::BoxFuture, FutureExt, StreamExt, TryFutureExt};
use ioc_rs::{Registered, ServiceCollection};
use sqlx::SqlitePool;
use std::sync::Arc;

pub fn register(collection: &mut ServiceCollection) {
    println!("Register SQL");
    collection.register(|| SqliteConfiguration {
        connection: "sqlite::memory:".to_owned(),
    });

    collection
        .with::<Registered<SqliteConfiguration>>()
        .register_shared(|config| {
            Arc::new(
                async move {
                    sqlx::sqlite::SqlitePoolOptions::new()
                        .connect(&config.connection)
                        .await
                }
                .map_err(|e| Arc::new(e))
                .shared(),
            )
        })
        .alias::<DynamicDatabaseExecutorPromise>(|i| {
            (*i).clone()
                .map(|pool_result| {
                    pool_result.map(|pool| Box::new(pool) as Box<dyn SqliteExecutor + Send + Sync>)
                })
                .boxed()
        });
    collection
        .with::<Registered<DynamicDatabaseExecutorPromise>>()
        .register(|connection| DynConnectionWrapper::new(connection));
}

impl SqliteExecutor for SqlitePool {
    fn execute<'e, 'q: 'e>(
        &'e self,
        q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    ) -> BoxFuture<'e, Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error>> {
        async move { q.execute(self).await }.boxed()
    }

    fn fetch_many<'e, 'c: 'e, 'q: 'e>(
        &'c self,
        q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    ) -> FetchManyResult<'e> {
        q.fetch_many(self).boxed()
    }
}
