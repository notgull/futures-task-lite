//! Boxed, dynamic executors.

use crate::Executor;

use alloc::boxed::Box;

use core::future::Future;
use core::pin::Pin;

/// A dynamic [`Executor`] allocated on the heap.
#[allow(clippy::type_complexity)]
pub struct BoxedExecutor<'a, T> {
    inner: Box<
        dyn Executor<
                Pin<Box<dyn Future<Output = T> + Send + 'a>>,
                Task = Pin<Box<dyn Future<Output = T> + Send + 'a>>,
                Error = Box<dyn std::error::Error + Send + 'a>,
            > + Send
            + 'a,
    >,
}

impl<'a, T> BoxedExecutor<'a, T> {
    /// Create a new `BoxedExecutor`.
    pub fn new<E: Executor<Pin<Box<dyn Future<Output = T> + Send + 'a>>> + Send + 'a>(
        exec: E,
    ) -> Self
    where
        E::Task: Send + 'a,
        E::Error: std::error::Error + Send + 'a,
    {
        // Inner executor that wraps the task in a box.
        struct BoxingExecutor<E>(E);

        impl<'a, T, E: Executor<Pin<Box<dyn Future<Output = T> + Send + 'a>>>>
            Executor<Pin<Box<dyn Future<Output = T> + Send + 'a>>> for BoxingExecutor<E>
        where
            E::Task: Send + 'a,
            E::Error: std::error::Error + Send + 'a,
        {
            type Task = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
            type Error = Box<dyn std::error::Error + Send + 'a>;

            fn try_spawn(
                &self,
                future: Pin<Box<dyn Future<Output = T> + Send + 'a>>,
            ) -> Result<Self::Task, Self::Error> {
                match self.0.try_spawn(future) {
                    Ok(task) => Ok(Box::pin(task)),
                    Err(err) => Err(Box::new(err)),
                }
            }
        }

        BoxedExecutor {
            inner: Box::new(BoxingExecutor(exec)),
        }
    }
}

impl<'a, T, F: Future<Output = T> + Send + 'a> Executor<F> for BoxedExecutor<'a, T> {
    type Task = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
    type Error = Box<dyn std::error::Error + Send + 'a>;

    fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
        self.inner.try_spawn(Box::pin(future))
    }
}

/// A dynamic [`Executor`] allocated on the heap, but thread-unsafe.
#[allow(clippy::type_complexity)]
pub struct LocalBoxedExecutor<'a, T> {
    inner: Box<
        dyn Executor<
                Pin<Box<dyn Future<Output = T> + 'a>>,
                Task = Pin<Box<dyn Future<Output = T> + 'a>>,
                Error = Box<dyn std::error::Error + 'a>,
            > + 'a,
    >,
}

impl<'a, T> LocalBoxedExecutor<'a, T> {
    /// Create a new `BoxedExecutor`.
    pub fn new<E: Executor<Pin<Box<dyn Future<Output = T> + 'a>>> + 'a>(exec: E) -> Self
    where
        E::Task: 'a,
        E::Error: std::error::Error + 'a,
    {
        // Inner executor that wraps the task in a box.
        struct BoxingExecutor<E>(E);

        impl<'a, T, E: Executor<Pin<Box<dyn Future<Output = T> + 'a>>>>
            Executor<Pin<Box<dyn Future<Output = T> + 'a>>> for BoxingExecutor<E>
        where
            E::Task: 'a,
            E::Error: std::error::Error + 'a,
        {
            type Task = Pin<Box<dyn Future<Output = T> + 'a>>;
            type Error = Box<dyn std::error::Error + 'a>;

            fn try_spawn(
                &self,
                future: Pin<Box<dyn Future<Output = T> + 'a>>,
            ) -> Result<Self::Task, Self::Error> {
                match self.0.try_spawn(future) {
                    Ok(task) => Ok(Box::pin(task)),
                    Err(err) => Err(Box::new(err)),
                }
            }
        }

        LocalBoxedExecutor {
            inner: Box::new(BoxingExecutor(exec)),
        }
    }
}

impl<'a, T, F: Future<Output = T> + 'a> Executor<F> for LocalBoxedExecutor<'a, T> {
    type Task = Pin<Box<dyn Future<Output = T> + 'a>>;
    type Error = Box<dyn std::error::Error + 'a>;

    fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
        self.inner.try_spawn(Box::pin(future))
    }
}
