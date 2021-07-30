use core::pin::Pin;
use either::Either;
use futures::future::BoxFuture;
use futures::stream::Stream;
use std::sync::Arc;

pub type DynamicDatabaseExecutorPromise = BoxFuture<'static, DynamicDatabaseExecutor>;
pub type DynamicDatabaseExecutor = Result<Box<dyn SqliteExecutor + Send + Sync>, Arc<sqlx::Error>>;
pub type FetchManyResult<'e> = Pin<
    Box<
        dyn Stream<
                Item = Result<
                    Either<sqlx::sqlite::SqliteQueryResult, sqlx::sqlite::SqliteRow>,
                    sqlx::Error,
                >,
            > + Send
            + 'e,
    >,
>;

#[cfg(feature = "service")]
pub mod service;

pub struct DynConnectionWrapper(DynConnectionWrapperState);

impl DynConnectionWrapper {
    pub fn new(connection: DynamicDatabaseExecutorPromise) -> Self {
        Self(DynConnectionWrapperState::Step1(connection))
    }
    pub async fn borrow_connected(
        &mut self,
    ) -> Result<&Box<dyn SqliteExecutor + Send + Sync>, &Arc<sqlx::Error>> {
        self.0.borrow_connected().await
    }
}

enum DynConnectionWrapperState {
    Step1(DynamicDatabaseExecutorPromise),
    Step2(DynamicDatabaseExecutor),
}

impl DynConnectionWrapperState {
    pub async fn borrow_connected(
        &mut self,
    ) -> Result<&Box<dyn SqliteExecutor + Send + Sync>, &Arc<sqlx::Error>> {
        if let DynConnectionWrapperState::Step1(x) = self {
            *self = DynConnectionWrapperState::Step2(x.await);
        }

        if let DynConnectionWrapperState::Step2(x) = self {
            return x.as_ref();
        }
        unreachable!("State must be in Step2")
    }
}

pub trait SqliteExecutor {
    fn execute<'e, 'q: 'e>(
        &'e self,
        q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    ) -> BoxFuture<'e, Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error>>;
    fn fetch_many<'e, 'c: 'e, 'q: 'e>(
        &'c self,
        q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    ) -> FetchManyResult<'e>;
}

pub struct SqliteConfiguration {
    pub connection: String,
}
