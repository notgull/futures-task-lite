//! Traits for spawning futures on an executor.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
mod boxed;
pub mod impls;

#[cfg(feature = "std")]
pub use boxed::{BoxedExecutor, LocalBoxedExecutor};

use core::convert::Infallible;
use core::future::Future;

/// Extension trait for a [`Future`].
///
/// [`Future`]: core::future::Future
pub trait FutureExt: Future + Sized {
    /// Spawn this future on an executor.
    fn try_par<E: Executor<Self>>(self, ex: E) -> Result<E::Task, E::Error> {
        ex.try_spawn(self)
    }

    // Spawn this future on an executor infallibly.
    fn par<E: InfallibleExecutor<Self>>(self, ex: E) -> E::Task {
        ex.spawn(self)
    }
}
impl<F: Future + Sized> FutureExt for F {}

/// Trait for an executor that [`Future`]s can be spawned onto.
pub trait Executor<F: Future> {
    /// The task type produced by spawning a future.
    ///
    /// It is assumed that dropping a task cancels it implicitly.
    type Task: Future<Output = F::Output>;

    /// The error type that can occur while spawning.
    type Error;

    /// Try to spawn the future on this executor.
    fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error>;
}

impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for &mut E {
    type Task = E::Task;
    type Error = E::Error;

    #[inline]
    fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
        (**self).try_spawn(future)
    }
}

impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for &E {
    type Task = E::Task;
    type Error = E::Error;

    #[inline]
    fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
        (**self).try_spawn(future)
    }
}

/// Trait for a task that can be canceled.
// TODO: GAT and TAIT
pub trait CancellableTask<'a>: Future + 'a {
    /// The future returned by trying to cancel this task.
    type Cancel: Future<Output = Option<Self::Output>> + 'a;

    /// Cancel this future.
    fn cancel(self) -> Self::Cancel;
}

/// Trait for a task that can be detached to run forever.
pub trait DetachableTask: Future {
    /// Detach this future and let it run forever.
    fn detach(self);
}

/// Executors that are infallible.
pub trait InfallibleExecutor<F: Future>: Executor<F, Error = Infallible> {
    /// Spawn a task infallibly.
    fn spawn(&self, future: F) -> Self::Task {
        match self.try_spawn(future) {
            Ok(task) => task,
            Err(infl) => match infl {},
        }
    }
}
impl<F: Future, E: Executor<F, Error = Infallible>> InfallibleExecutor<F> for E {}

#[cfg(feature = "alloc")]
mod alloc_impls {
    use super::Executor;
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::sync::Arc;
    use core::future::Future;

    impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for Box<E> {
        type Task = E::Task;
        type Error = E::Error;

        #[inline]
        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            (**self).try_spawn(future)
        }
    }

    impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for Rc<E> {
        type Task = E::Task;
        type Error = E::Error;

        #[inline]
        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            (**self).try_spawn(future)
        }
    }

    impl<F: Future, E: Executor<F> + ?Sized> Executor<F> for Arc<E> {
        type Task = E::Task;
        type Error = E::Error;

        #[inline]
        fn try_spawn(&self, future: F) -> Result<Self::Task, Self::Error> {
            (**self).try_spawn(future)
        }
    }
}
