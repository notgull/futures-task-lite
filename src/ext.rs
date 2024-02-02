//! Extension trait for executors.

use crate::Executor;

use alloc::vec;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use pin_project_lite::pin_project;

/// Additional methods for the [`Executor`] trait.
///
/// [`Executor`]: crate::Executor
pub trait ExecutorExt<F: Future>: Executor<F> {
    /// Poll a series of futures in parallel.
    fn all(&self, futures: impl IntoIterator<Item = F>) -> AllFuture<'_, F, Self> {
        // Collect the tasks into a vector.
        let result = futures
            .into_iter()
            .map(|future| self.try_spawn(future))
            .collect::<Result<Vec<_>, _>>();

        match result {
            Ok(tasks) => AllFuture {
                error: None,
                executor: self,
                tasks: tasks.into_iter(),
                current: None,
            },

            Err(err) => AllFuture {
                error: Some(err),
                executor: self,
                tasks: vec::Vec::new().into_iter(),
                current: None,
            },
        }
    }
}

impl<F: Future, E: Executor<F>> ExecutorExt<F> for E {}

pin_project! {
    /// Future for the `all` method.
    pub struct AllFuture<'a, F: Future, E: Executor<F>> where E: ?Sized {
        // An error occurred while spawning tasks.
        error: Option<E::Error>,

        // The executor.
        executor: &'a E,

        // The list of tasks to poll.
        tasks: vec::IntoIter<E::Task>,

        // The task we are currently polling.
        #[pin]
        current: Option<E::Task>,
    }
}

impl<F: Future, E: Executor<F> + ?Sized> Future for AllFuture<'_, F, E> {
    type Output = Result<(), E::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        // If there is an error, return early.
        if let Some(error) = this.error.take() {
            return Poll::Ready(Err(error));
        }

        loop {
            // Poll the current task, if any.
            if let Some(current) = this.current.as_mut().as_pin_mut() {
                if current.poll(cx).is_ready() {
                    // This task is finished, set up for the next one.
                    this.current.as_mut().set(None);
                } else {
                    return Poll::Pending;
                }
            }

            // See if there is another task to poll.
            match this.tasks.next() {
                Some(task) => {
                    // There is, recurse.
                    this.current.as_mut().set(Some(task));
                }
                None => return Poll::Ready(Ok(())),
            }
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
