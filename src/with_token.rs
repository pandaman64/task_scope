use pin_project::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::waker;
use crate::Token;

// a future wrapper polls the inner future with the given cancellation token embedded in the context
#[pin_project]
pub(crate) struct WithToken<Fut> {
    #[pin]
    inner: Fut,
}

impl<Fut> WithToken<Fut> {
    pub(crate) fn new(inner: Fut) -> Self {
        Self { inner }
    }
}

impl<Fut> WithToken<Fut>
where
    Fut: Future,
{
    pub(crate) fn poll(self: Pin<&mut Self>, cx: &mut Context, token: Token) -> Poll<Fut::Output> {
        let original = cx.waker().clone();
        let waker = unsafe { waker::waker(token, original) };
        let mut new_cx = Context::from_waker(&waker);

        self.project().inner.poll(&mut new_cx)
    }
}
