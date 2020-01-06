use futures::future::FusedFuture;
use std::future::Future;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::handle::{JoinHandle, RuntimeJoinHandle};
use crate::spawn::install;
use crate::Canceled;

impl<T> RuntimeJoinHandle for tokio::task::JoinHandle<Result<T, Canceled>>
where
    T: Send + 'static,
{
    type Output = T;
    type Error = tokio::task::JoinError;
}

/// A future that spawns the given future on the first poll.
///
/// This future is created by [`spawn`] function and avaiable when `"tokio"` feature is enabled.
#[must_use = "do not spawn unless polled"]
pub struct SpawnFuture<F>(Option<F>);

impl<F> Unpin for SpawnFuture<F> {}

impl<T, F> Future for SpawnFuture<F>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    type Output = JoinHandle<tokio::task::JoinHandle<Result<T, Canceled>>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let future = self.0.take().expect("already spawned");
        let inner = tokio::spawn(install(future, cx));

        Poll::Ready(JoinHandle { inner })
    }
}

impl<F> FusedFuture for SpawnFuture<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn is_terminated(&self) -> bool {
        self.0.is_none()
    }
}

/// Creates a future that spawns the given task in the current scope.
///
/// This function is available when `"tokio"` feature is enabled.
///
/// `spawn` function does not spawn a new task immediately. Instead, it returns a future for
/// spawning the given task. Currently, the returned future will spawn the task at the first
/// poll, but future versions may defer the spawn (e.g. rate limiting new tasks).
///
/// A graceful cancellation request will be propagated to the spawned task. If the scope is
/// forcibly canceled, the spawned task will not be resumed after the next cancellation point.
///
/// # Returns
/// The returned future resolves to [`JoinHandle`] for the spawned task.
/// `JoinHandle` can be used to wait for the completion of the task.
///
/// # Panics
/// The returned future panics when polled **outside** of a scope or after the spawning.
pub fn spawn<F>(task: F) -> SpawnFuture<F> {
    SpawnFuture(Some(task))
}
