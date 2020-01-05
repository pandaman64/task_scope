use futures::future::{FusedFuture, OptionFuture};
use futures_intrusive::channel::{shared::StateReceiveFuture, StateId};
use pin_project::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::waker;
use crate::Canceled;

/// Future for checking the current cancellation status (graceful, forced).
///
/// This future is created by [`cancellation`] function.
#[must_use = "futures do nothing unless polled"]
#[pin_project]
pub struct Cancellation {
    #[pin]
    inner: OptionFuture<StateReceiveFuture<parking_lot::RawMutex, bool>>,
    state_id: StateId,
}

impl Cancellation {
    pub(crate) fn none() -> Self {
        Self {
            inner: None.into(),
            state_id: StateId::new(),
        }
    }
}

impl Future for Cancellation {
    type Output = Option<Canceled>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.project();

        // TODO: poll this.cancel before updating?

        // update the scope
        this.inner
            .as_mut()
            .set(waker::cancellation(cx, *this.state_id));

        this.inner.as_mut().poll(cx).map(|v| match v {
            Some(Some((state_id, true))) => {
                *this.state_id = state_id;
                Some(Canceled::Graceful)
            }
            Some(Some((_, false))) => unreachable!(),
            Some(None) => Some(Canceled::Forced),
            None => None,
        })
    }
}

impl FusedFuture for Cancellation {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

/// Creates a future that checks the current cancellation status.
///
/// The returned future resolves to:
/// - `Some(e)` when the cancellation is in progress. The error object indicates whether the
/// cancellation is graceful or forced.
/// - `None` when the cancellation is not supported in the current context.
///
/// # Notes
/// Normally, a task should not await the future, because the task will be blocked until someone
/// issues a cancellation. Instead, try polling the cancellation future regularly:
///
/// ```no_run
/// # async fn doc_cancellation() -> Result<(), String> {
/// use futures::{pin_mut, poll};
/// use task_scope::cancellation;
/// use std::task::Poll;
///
/// let cancellation = cancellation();
/// pin_mut!(cancellation);
///
/// loop {
///     if let Poll::Ready(Some(e)) = poll!(cancellation.as_mut()) {
///         // run cancellation logic
///
///         return Err("canceled".into());
///     }
///
///     // normal operations
/// }
/// # }
/// ```
///
/// You can wrap `Future`/`AsyncRead`/`AsyncWrite` using [`cancelable`] function, which
/// automatically stops the underlying operation on a cancellation.
///
/// [`cancelable`]: crate::cancelable()
///
/// The cancellation future should be stored in the tasks. Otherwise, the cancellation might not
/// wake them up.
pub fn cancellation() -> Cancellation {
    Cancellation::none()
}
