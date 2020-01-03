use futures::future::poll_fn;
use futures::pin_mut;

use std::future::Future;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};

use crate::handle::JoinHandle;
use crate::with_token::WithToken;
use crate::Token;
use crate::{waker, Canceled};

pub struct SpawnFuture<F>(Option<F>);

impl<F> Unpin for SpawnFuture<F> {}

impl<F> Future for SpawnFuture<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    type Output = Result<JoinHandle<F::Output>, Canceled>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let fut = self.0.take().expect("already spawned");
        let data = unsafe { waker::retrieve_data(cx).expect("must be polled in a scope") };
        match Weak::upgrade(&data.token.join) {
            Some(join) => {
                let cancel = data.token.cancel.clone();
                let inner = tokio::spawn(async move {
                    let fut = WithToken::new(crate::cancelable(fut));
                    pin_mut!(fut);

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

                    ret
                });

                Poll::Ready(Ok(JoinHandle { inner }))
            }
            None => Poll::Ready(Err(Canceled)),
        }
    }
}

pub fn spawn<F>(fut: F) -> SpawnFuture<F> {
    SpawnFuture(Some(fut))
}
