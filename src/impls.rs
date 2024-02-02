//! Implementations for foreign crates.

#[cfg(feature = "async-task")]
mod async_task_impl {
    use crate::{CancellableTask, DetachableTask};
    use async_task_crate::{FallibleTask, Task};

    use core::future::Future;
    use core::pin::Pin;

    use alloc::boxed::Box;

    impl<'a, T: Send + 'a, M: Send + Sync + 'a> CancellableTask<'a> for Task<T, M> {
        type Cancel = Pin<Box<dyn Future<Output = Option<T>> + Send + 'a>>;

        fn cancel(self) -> Self::Cancel {
            Box::pin(Task::cancel(self))
        }
    }

    impl<T, M> DetachableTask for Task<T, M> {
        fn detach(self) {
            Task::detach(self)
        }
    }

    impl<'a, T: Send + 'a, M: Send + Sync + 'a> CancellableTask<'a> for FallibleTask<T, M> {
        type Cancel = Pin<Box<dyn Future<Output = Option<Option<T>>> + Send + 'a>>;

        fn cancel(self) -> Self::Cancel {
            Box::pin(async move {
                let result = FallibleTask::cancel(self).await;
                Some(result)
            })
        }
    }

    impl<T, M> DetachableTask for FallibleTask<T, M> {
        fn detach(self) {
            FallibleTask::detach(self)
        }
    }
}

#[cfg(feature = "async-executor")]
mod async_executor_impl {
    use crate::Executor;
    use async_executor_crate::{LocalExecutor, Task};

    use core::convert::Infallible;
    use core::future::Future;

    impl<'a, F: Future + Send + 'a> Executor<F> for async_executor_crate::Executor<'a>
    where
        F::Output: Send + 'a,
    {
        type Task = Task<F::Output>;
        type Error = Infallible;

        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            Ok(self.spawn(future))
        }
    }

    impl<'a, F: Future + 'a> Executor<F> for LocalExecutor<'a>
    where
        F::Output: 'a,
    {
        type Task = Task<F::Output>;
        type Error = Infallible;

        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            Ok(self.spawn(future))
        }
    }
}

#[cfg(feature = "tokio")]
mod tokio_impl {
    use crate::{CancellableTask, DetachableTask, Executor};
    use tokio::runtime::{Handle, Runtime};
    use tokio::task::JoinHandle;

    use core::convert::Infallible;
    use core::future::{ready, Future, Ready};
    use core::pin::Pin;
    use core::task::{Context, Poll};

    /// Implements traits for `tokio`'s global runtime.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct TokioGlobal {
        _private: (),
    }

    impl<F: Future + Send + 'static> Executor<F> for TokioGlobal
    where
        F::Output: Send + 'static,
    {
        type Task = TokioTask<F::Output>;
        type Error = tokio::runtime::TryCurrentError;

        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            Handle::try_current().map(|handle| match handle.try_spawn(future) {
                Ok(task) => task,
                Err(infl) => match infl {},
            })
        }
    }

    /// A wrapper around a [`tokio::task::JoinHandle`] with task semantics.
    pub struct TokioTask<T>(Option<JoinHandle<T>>);

    impl<T> TokioTask<T> {
        /// Get a reference to the inner `JoinHandle`.
        pub fn get_ref(&self) -> &JoinHandle<T> {
            self.0.as_ref().unwrap()
        }

        /// Get a mutable reference to the inner `JoinHandle`.
        pub fn get_mut(&mut self) -> &mut JoinHandle<T> {
            self.0.as_mut().unwrap()
        }

        /// Convert to the inner join handle.
        pub fn into_inner(mut self) -> JoinHandle<T> {
            self.0.take().unwrap()
        }
    }

    impl<T> Future for TokioTask<T> {
        type Output = T;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match Pin::new(self.0.as_mut().unwrap()).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Ok(x)) => {
                    self.0 = None;
                    Poll::Ready(x)
                }
                Poll::Ready(Err(err)) => {
                    self.0 = None;
                    panic!("task was cancelled or panicked: {}", err)
                }
            }
        }
    }

    impl<'a, T: 'a> CancellableTask<'a> for TokioTask<T> {
        type Cancel = Ready<Option<T>>;

        fn cancel(mut self) -> Self::Cancel {
            self.get_mut().abort();

            // TODO: Poll self once noop_waker is stable.
            ready(None)
        }
    }

    impl<T> DetachableTask for TokioTask<T> {
        fn detach(mut self) {
            // Dropping the tokio task automatically detaches it.
            self.0 = None;
        }
    }

    impl<T> Drop for TokioTask<T> {
        fn drop(&mut self) {
            if let Some(task) = self.0.take() {
                task.abort();
            }
        }
    }

    impl<F: Future + Send + 'static> Executor<F> for Handle
    where
        F::Output: Send + 'static,
    {
        type Error = Infallible;
        type Task = TokioTask<F::Output>;

        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            Ok(TokioTask(Some(self.spawn(future))))
        }
    }

    impl<F: Future + Send + 'static> Executor<F> for Runtime
    where
        F::Output: Send + 'static,
    {
        type Error = Infallible;
        type Task = TokioTask<F::Output>;

        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            Ok(TokioTask(Some(self.spawn(future))))
        }
    }
}

#[cfg(feature = "tokio")]
pub use tokio_impl::{TokioGlobal, TokioTask};
