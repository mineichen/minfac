use crate::TodoItem;
use anyhow::Result;
use futures::{future::BoxFuture, stream::BoxStream, FutureExt, StreamExt, TryStreamExt};
use ioc_rs::{Registered, ServiceCollection};
use raf_hosted_service::HostedService;
use raf_sql::{DynConnectionWrapper, DynamicDatabaseExecutorPromise};
use sqlx::FromRow;
use std::sync::Arc;

/// Receive all todo-items
pub trait TodoRepository: Send {
    fn get_all(&mut self) -> AllTodoItemsResult;
}

pub type AllTodoItemsResult<'a> = BoxFuture<'a, Result<TodoItemRow<'a>, Arc<sqlx::Error>>>;
pub type TodoItemRow<'a> = BoxStream<'a, Result<TodoItem, sqlx::Error>>;

pub(crate) fn register_services(collection: &mut ServiceCollection) {
    collection
        .with::<Registered<raf_sql::DynConnectionWrapper>>()
        .register(|conn| Box::new(SqlTodoRepository::new(conn)) as Box<dyn TodoRepository>);
    collection
        .with::<Registered<DynamicDatabaseExecutorPromise>>()
        .register(|x| Box::new(Migration(x)) as Box<dyn HostedService>);
}

struct SqlTodoRepository {
    connection: DynConnectionWrapper,
}

impl SqlTodoRepository {
    fn new(connection: raf_sql::DynConnectionWrapper) -> Self {
        SqlTodoRepository { connection }
    }
}

#[derive(sqlx::FromRow, Debug)]
struct SqlTodoItem {
    id: i32,
    title: String,
}

impl From<SqlTodoItem> for TodoItem {
    fn from(input: SqlTodoItem) -> Self {
        Self {
            id: input.id,
            title: input.title,
        }
    }
}

impl TodoRepository for SqlTodoRepository {
    fn get_all(&mut self) -> AllTodoItemsResult {
        async move {
            let pool = self
                .connection
                .borrow_connected()
                .await
                .map_err(|e| e.clone())?;

            Ok(pool
                .fetch_many(sqlx::query("SELECT * FROM todo"))
                .try_filter_map(|f| async move {
                    match f.right() {
                        Some(x) => {
                            let item = SqlTodoItem::from_row(&x)?;
                            Ok(Some(TodoItem {
                                id: item.id,
                                title: item.title,
                            }))
                        }
                        None => Ok(None),
                    }
                })
                .boxed())
        }
        .boxed()
    }
}

struct Migration(DynamicDatabaseExecutorPromise);

impl HostedService for Migration {
    fn start(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        async {
            setup_database(self.0).await?;
            Ok(())
        }
        .boxed()
    }
}

async fn setup_database(pool: DynamicDatabaseExecutorPromise) -> Result<()> {
    let pool = pool.await?;

    pool.execute(sqlx::query(
        "CREATE TABLE todo (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL
    )",
    ))
    .await?;

    pool.execute(sqlx::query("INSERT INTO todo (title) VALUES (?)").bind("Do something"))
        .await?;

    pool.execute(sqlx::query("INSERT INTO todo (title) VALUES (?)").bind("Do something else"))
        .await?;

    let mut s = pool
        .fetch_many(sqlx::query("SELECT * FROM todo"))
        .try_filter_map(|f| async move {
            match f.right() {
                Some(x) => Ok(Some(SqlTodoItem::from_row(&x)?)),
                None => Ok(None),
            }
        })
        .boxed();

    while let Some(x) = s.next().await {
        println!("From Database: {:?}", x);
    }

    Ok(())
}
