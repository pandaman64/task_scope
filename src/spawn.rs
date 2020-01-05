use futures::future::{poll_fn, FusedFuture};
use futures::{pin_mut, poll};

use std::future::Future;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};

use crate::handle::JoinHandle;
use crate::with_token::WithToken;
use crate::{cancellation, waker, Canceled, Token};

/// A future that spawns the given future on the first poll.
///
/// This future is created by [`spawn`] function.
#[must_use = "do not spawn unless polled"]
pub struct SpawnFuture<F>(Option<F>);

impl<F> Unpin for SpawnFuture<F> {}

impl<F> Future for SpawnFuture<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    type Output = JoinHandle<F::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let fut = self.0.take().expect("already spawned");
        let data = unsafe { waker::retrieve_data(cx).expect("must be polled in a scope") };
        let join = Weak::upgrade(&data.token.join).expect("no task is running");

        let cancel = data.token.cancel.clone();
        let inner = tokio::spawn(async move {
            let fut = WithToken::new(fut);
            pin_mut!(fut);

            let cancellation = cancellation();
            pin_mut!(cancellation);

            // stop the task only if a forceful cancellation is issued.
            // currently, the children can continue running on a graceful cancellation
            // so that they can perform custom cancellation logic
            //
            // TODO: add a builder API (and a helper) for automatically canceling
            // the inner future on a graceful cancellation
            if let Poll::Ready(Some(Canceled::Forced)) = poll!(cancellation) {
                return Err(Canceled::Forced);
            }

            let ret = poll_fn(|cx| {
                let token = Token {
                    cancel: cancel.clone(),
                    join: Arc::downgrade(&join),
                };
                fut.as_mut().poll(cx, token)
            })
            .await;

            drop(join);
            drop(cancel);

            Ok(ret)
        });

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
