//! Traits for spawning futures on an executor.

#[cfg(feature = "alloc")]
extern crate alloc;

use core::future::Future;

/// Trait for an executor that [`Future`]s can be spawned onto.
// TODO: Replace `F` here with a GAT once 1.65 is available on Debian Stable.
pub trait Executor<F: Future> {
    /// The task type produced by spawning a future.
    /// 
    /// It is assumed that dropping a task cancels it implicitly.
    type Task: Future<Output = F::Output>;

    /// The error type that can occur while spawning.
    type Error;

    /// Try to spawn the future on this executor.
    fn try_spawn(&mut self, future: F) -> Result<Self::Task, Self::Error>;
}

impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for &mut E {
    type Task = E::Task;
    type Error = E::Error;

    #[inline]
    fn try_spawn(&mut self, future: F) -> Result<Self::Task, Self::Error> {
        (**self).try_spawn(future)
    }
}

/// Trait for a task that can be canceled.
pub trait CancellableTask: Future {
    /// The future returned by trying to cancel this task.
    type Cancel: Future<Output = Option<Self::Output>>;

    /// Cancel this future.
    fn cancel(self) -> Self::Cancel;
}

/// Trait for a task that can be detached to run forever.
pub trait DetachableTask: Future {
    /// Detach this future and let it run forever.
    fn detach(self);
}

#[cfg(feature = "alloc")]
mod alloc_impls {
    use super::Executor;
    use core::future::Future;
    use alloc::boxed::Box;

    impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for Box<E> {
        type Task = E::Task;
        type Error = E::Error;

        #[inline]
        fn try_spawn(&mut self, future: F) -> Result<Self::Task, Self::Error> {
            (**self).try_spawn(future)            
        }
    }
}
