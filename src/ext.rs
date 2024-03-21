//! Extension trait for executors.

extern crate std; // TODO

use crate::{CancellableTask, Executor};

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use pin_project_lite::pin_project;



/// Poll a series of futures in parallel, using a collection for the tasks.
pub async fn all<F: Future, E: Executor<F>>(
    exec: E,
    futures: impl IntoIterator<Item = F>,
    outputs: &mut impl Extend<F::Output>,
) -> Result<(), E::Error> {
    // Collect the tasks into a vector.
    let tasks = futures
        .into_iter()
        .map(|future| exec.try_spawn(future))
        .collect::<Result<Vec<_>, _>>()?;

    // Poll for all of the outputs.
    for task in tasks {
        outputs.extend(Some(task.await));
    }

    Ok(())
}

/// Poll a series of futures in parallel, spawning no more than `limit` at a time.
pub async fn all_limited<F: Future, E: Executor<SemaphoreFuture<F>>>(
    exec: E,
    futures: impl IntoIterator<Item = F>,
    outputs: &mut impl Extend<F::Output>,
    limit: usize,
) -> Result<(), E::Error> {
    let mut tasks = Vec::new();
    let semaphore = Arc::new(async_lock::Semaphore::new(limit));

    for future in futures {
        // Wait for a new semaphore guard to be available.
        let guard = semaphore.acquire_arc().await;

        // Spawn a future that runs, then drops the semaphore guard.
        let task = exec.try_spawn(SemaphoreFuture {
            inner: future,
            _guard: guard,
        })?;

        // Push the task.
        tasks.push(task);
    }

    // Collect the results.
    for task in tasks {
        outputs.extend(Some(task.await));
    }

    Ok(())
}

/// Race a set of futures together.
pub async fn or<'cancel, F: Future, E: Executor<SenderFuture<F>>>(
    exec: E,
    futures: impl IntoIterator<Item = F>,
) -> Result<F::Output, E::Error>
where
    E::Task: CancellableTask<'cancel>,
{
    let (sender, receiver) = async_channel::bounded(1);
    let mut tasks = Vec::new();

    // Spawn all of the tasks with a future that sends its output after.
    for future in futures {
        let task = exec.try_spawn(SenderFuture {
            inner: future,
            channel: sender.clone(),
        })?;
        tasks.push(task);
    }

    // Wait for one of the tasks to complete.
    let completed = async move {
        receiver
            .recv()
            .await
            .expect("all of the racing futures panicked")
    }
    .await;

    // Cancel all of the tasks.
    for task in tasks {
        task.cancel().await;
    }

    Ok(completed)
}

pin_project! {
    /// A future that wraps another future, then drops a semaphore.
    #[doc(hidden)]
    pub struct SemaphoreFuture<F: Future> {
        // The inner future.
        #[pin]
        inner: F,

        // Guard for the semaphore.
        _guard: async_lock::SemaphoreGuardArc
    }
}

impl<F: Future> Future for SemaphoreFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

pin_project! {
    /// A future that wraps another and then sends it on.
    #[doc(hidden)]
    pub struct SenderFuture<F: Future> {
        // The inner future.
        #[pin]
        inner: F,

        // The channel to send to.
        channel: async_channel::Sender<F::Output>
    }
}

impl<F: Future> Future for SenderFuture<F> {
    type Output = Result<(), async_channel::TrySendError<F::Output>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.inner.poll(cx) {
            Poll::Ready(item) => Poll::Ready(this.channel.try_send(item)),
            Poll::Pending => Poll::Pending,
        }
    }
}

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
