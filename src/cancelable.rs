use futures::future::OptionFuture;
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::cancellation_future::CancellationFuture;
use crate::waker;
use crate::Canceled;

#[pin_project]
pub struct Cancelable<T> {
    #[pin]
    inner: OptionFuture<T>,
    #[pin]
    cancel: OptionFuture<CancellationFuture>,
}

// these impls are based on futures' Abortable
impl<T> Future for Cancelable<T>
where
    T: Future,
{
    type Output = Result<T::Output, Canceled>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.project();

        // TODO: poll this.cancel before updating?

        // update the scope
        this.cancel.set(waker::cancellation_future(cx));

        // check if the scope is canceled
        if let Poll::Ready(Some(())) = this.cancel.poll(cx) {
            // stop the inner future
            // TODO: we may want to cancel the inner future gracefully
            this.inner.set(None.into());
            return Poll::Ready(Err(Canceled));
        }

        match this.inner.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(v)) => Poll::Ready(Ok(v)),
            Poll::Ready(None) => panic!("poll after cancellation"),
        }
    }
}

pub fn cancelable<T>(v: T) -> Cancelable<T> {
    Cancelable {
        inner: Some(v).into(),
        cancel: None.into(),
    }
}
