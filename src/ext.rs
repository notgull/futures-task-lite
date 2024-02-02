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
