//! Extension trait for executors.

use crate::{Executor, CancellableTask, DetachableTask};

use core::future::Future;

/// Additional methods for the [`Executor`] trait.
/// 
/// [`Executor`]: crate::Executor
pub trait ExecutorExt<F: Future> : Executor<F> {
}

impl<F: Future, E: Executor<F>> ExecutorExt<F> for E {}
