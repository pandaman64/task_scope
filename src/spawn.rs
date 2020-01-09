use futures::future::poll_fn;
use futures::pin_mut;

use std::future::Future;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};

use crate::with_token::WithToken;
use crate::{cancellation, waker, Canceled, Token};

#[cfg(feature = "async-std")]
pub mod async_std;
#[cfg(feature = "async-std")]
pub use self::async_std::*;
#[cfg(feature = "tokio")]
pub mod tokio;
#[cfg(feature = "tokio")]
pub use self::tokio::*;

/// Installs the current context in the given future.
///
/// The returned future receives a cancellation signal from the current context and terminates when
/// it encounters a forced cancellation.
pub fn install<'f, T, F>(
    future: F,
    cx: &mut Context,
) -> impl Future<Output = Result<T, Canceled>> + 'f
where
    F: Future<Output = T> + 'f,
{
    let data = unsafe { waker::retrieve_data(cx).expect("must be polled in a scope") };
    let join = Weak::upgrade(&data.token.join).expect("no task is running");
    let cancel = data.token.cancel.clone();

    async move {
        // introduce cancellation points at every yield
        let future = WithToken::new(async move {
            pin_mut!(future);

            let cancellation = cancellation();
            pin_mut!(cancellation);

            poll_fn(|cx| {
                // stop the task only if a forced cancellation is issued
                // the tasks can continue running on a graceful cancellation
                // so that they can perform custom cancellation logic
                if let Poll::Ready(Some(Canceled::Forced)) = cancellation.as_mut().poll(cx) {
                    return Poll::Ready(Err(Canceled::Forced));
                }

                future.as_mut().poll(cx).map(Ok)
            })
            .await
        });
        pin_mut!(future);

        poll_fn(|cx| {
            let token = Token {
                cancel: cancel.clone(),
                join: Arc::downgrade(&join),
            };
            future.as_mut().poll(cx, token)
        })
        .await
    }
}
