use pin_project::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::scope::Cancellation;
use crate::waker;

#[pin_project]
pub(crate) struct WithCancellation<Fut> {
    #[pin]
    pub(crate) inner: Fut,
}

impl<Fut> WithCancellation<Fut> {
    pub(crate) fn new(inner: Fut) -> Self {
        Self { inner }
    }
}

impl<Fut> WithCancellation<Fut>
where
    Fut: Future,
{
    pub(crate) fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context,
        cancellation: Cancellation,
    ) -> Poll<Fut::Output> {
        let original = cx.waker().clone();
        let waker = unsafe { waker::waker(cancellation, original) };
        let mut new_cx = Context::from_waker(&waker);

        self.project().inner.poll(&mut new_cx)
    }
}
