//! Shared state primitives for Gestalt Swarm
//!
//! Reduces Arc<RwLock<T>> boilerplate with ergonomic wrappers.

use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Thread-safe shared state wrapper
#[derive(Debug)]
pub struct SharedState<T: Send + Sync> {
    inner: Arc<RwLock<T>>,
}

impl<T: Send + Sync> SharedState<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<T> {
        self.inner.read().await
    }

    pub async fn write(&self) -> RwLockWriteGuard<T> {
        self.inner.write().await
    }

    pub fn into_arc(self) -> Arc<RwLock<T>> {
        self.inner
    }

    pub fn arc(&self) -> Arc<RwLock<T>> {
        self.inner.clone()
    }
}

impl<T: Send + Sync> Clone for SharedState<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Default + Send + Sync> Default for SharedState<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
