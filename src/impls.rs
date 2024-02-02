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
