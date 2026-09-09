use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use sqlx::{pool::PoolConnection, Sqlite, SqliteConnection, Transaction};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::SqlitePool;

type SharedTransaction = Arc<Mutex<Option<Transaction<'static, Sqlite>>>>;

#[derive(Debug)]
struct BackupTransactionState {
    transaction: SharedTransaction,
    active: AtomicBool,
}

#[derive(Debug, Clone)]
enum ConnectionSource {
    Pool(SqlitePool),
    Transaction(Weak<BackupTransactionState>),
}

/// Explicitly selects either a pooled connection or a backup's shared transaction.
///
/// Transaction sources do not keep their owning backup transaction alive. Once
/// the owner is committed, rolled back, or dropped, acquiring a connection fails.
#[derive(Debug, Clone)]
pub struct SqliteConnectionSource {
    inner: ConnectionSource,
}

impl From<SqlitePool> for SqliteConnectionSource {
    fn from(pool: SqlitePool) -> Self {
        Self {
            inner: ConnectionSource::Pool(pool),
        }
    }
}

impl From<&SqlitePool> for SqliteConnectionSource {
    fn from(pool: &SqlitePool) -> Self {
        pool.clone().into()
    }
}

impl SqliteConnectionSource {
    /// Acquires exclusive access to the connection for one operation.
    ///
    /// Release the guard before calling another repository method using this
    /// source. Nested SQL transactions must begin on the guarded connection so
    /// SQLx creates savepoints inside the owning backup transaction.
    pub async fn acquire(&self) -> Result<SqliteConnectionGuard, sqlx::Error> {
        let inner = match &self.inner {
            ConnectionSource::Pool(pool) => ConnectionGuard::Pool(pool.acquire().await?),
            ConnectionSource::Transaction(state) => {
                let state = state.upgrade().ok_or_else(transaction_closed)?;
                if !state.active.load(Ordering::Acquire) {
                    return Err(transaction_closed());
                }
                let transaction = state.transaction.clone().lock_owned().await;
                if !state.active.load(Ordering::Acquire) || transaction.is_none() {
                    return Err(transaction_closed());
                }
                ConnectionGuard::Transaction(transaction)
            }
        };
        Ok(SqliteConnectionGuard { inner })
    }
}

#[derive(Debug)]
enum ConnectionGuard {
    Pool(PoolConnection<Sqlite>),
    Transaction(OwnedMutexGuard<Option<Transaction<'static, Sqlite>>>),
}

/// Exclusive access to the selected SQLite connection.
#[derive(Debug)]
pub struct SqliteConnectionGuard {
    inner: ConnectionGuard,
}

impl Deref for SqliteConnectionGuard {
    type Target = SqliteConnection;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            ConnectionGuard::Pool(connection) => connection,
            ConnectionGuard::Transaction(transaction) => transaction
                .as_deref()
                .expect("an acquired backup connection has an active transaction"),
        }
    }
}

impl DerefMut for SqliteConnectionGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.inner {
            ConnectionGuard::Pool(connection) => connection,
            ConnectionGuard::Transaction(transaction) => transaction
                .as_deref_mut()
                .expect("an acquired backup connection has an active transaction"),
        }
    }
}

/// Owns the single database transaction used by a complete backup operation.
///
/// Dropping this owner closes its sources immediately. SQLx rolls back when the
/// last in-flight connection guard is released, including after cancellation.
#[derive(Debug)]
pub struct SqliteBackupTransaction {
    state: Arc<BackupTransactionState>,
}

impl SqliteBackupTransaction {
    pub async fn begin(pool: &SqlitePool, write: bool) -> Result<Self, sqlx::Error> {
        let mut transaction = if write {
            pool.begin_with("BEGIN IMMEDIATE").await?
        } else {
            pool.begin().await?
        };
        if write {
            sqlx::query("PRAGMA defer_foreign_keys = ON")
                .execute(&mut *transaction)
                .await?;
        }
        Ok(Self {
            state: Arc::new(BackupTransactionState {
                transaction: Arc::new(Mutex::new(Some(transaction))),
                active: AtomicBool::new(true),
            }),
        })
    }

    pub fn source(&self) -> SqliteConnectionSource {
        SqliteConnectionSource {
            inner: ConnectionSource::Transaction(Arc::downgrade(&self.state)),
        }
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.take_transaction().await?.commit().await
    }

    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.take_transaction().await?.rollback().await
    }

    async fn take_transaction(&self) -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        self.state.active.store(false, Ordering::Release);
        self.state
            .transaction
            .lock()
            .await
            .take()
            .ok_or_else(transaction_closed)
    }
}

impl Drop for SqliteBackupTransaction {
    fn drop(&mut self) {
        self.state.active.store(false, Ordering::Release);
    }
}

fn transaction_closed() -> sqlx::Error {
    sqlx::Error::Protocol("the backup transaction is no longer active".to_string())
}
