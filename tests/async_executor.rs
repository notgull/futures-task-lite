//! Tests using `async-executor`.

#![cfg(all(feature = "ext", feature = "async-executor"))]

use async_executor_crate::{Executor, LocalExecutor};
use futures_lite::future::{block_on, ready, yield_now};
use futures_task_lite::{all, all_limited, or, BoxedExecutor};

use std::cell::Cell;
use std::sync::Arc;

#[test]
fn test_all() {
    let ex = Executor::new();
    block_on(ex.run(async {
        let futures = [ready(1), ready(2), ready(3)];
        let mut results = Vec::new();

        all(&ex, futures, &mut results).await.unwrap();

        assert_eq!(results, [1, 2, 3]);
    }));
}

#[test]
fn test_all_limited() {
    let count = Cell::new(0);
    let ex = LocalExecutor::new();

    block_on(ex.run(async {
        let limited = |x| {
            let count = &count;
            async move {
                count.set(count.get() + 1);
                if count.get() >= 3 {
                    panic!("count exceeded three at a time");
                }

                // Stop for a second.
                yield_now().await;

                count.set(count.get() - 1);
                x
            }
        };

        let futures = [limited(1), limited(2), limited(3)];
        let mut results = Vec::new();

        all_limited(&ex, futures, &mut results, 2).await.unwrap();

        assert_eq!(results, [1, 2, 3]);
    }));
}

#[test]
fn test_or() {
    let finished = Cell::new(0);
    let ex = LocalExecutor::new();

    block_on(ex.run(async {
        let racey = |x| {
            let finished = &finished;
            async move {
                finished.set(x);
                x
            }
        };

        let futures = [racey(1), racey(2), racey(3)];
        let _result = or(&ex, futures).await.unwrap();

        assert_ne!(finished.get(), 0);
    }));
}

#[test]
fn test_all_on_boxed() {
    let ex = Arc::new(Executor::new());
    block_on(ex.run(async {
        let boxed = BoxedExecutor::new(ex.clone());

        let futures = [ready(1), ready(2), ready(3)];
        let mut results = Vec::new();

        all(&boxed, futures, &mut results).await.unwrap();

        assert_eq!(results, [1, 2, 3]);
    }));
}
