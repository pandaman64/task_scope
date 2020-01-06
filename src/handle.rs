use pin_project::pin_project;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::Canceled;

/// JoinHandle provided by the inner runtime
pub trait RuntimeJoinHandle:
    Future<
    Output = Result<
        Result<<Self as RuntimeJoinHandle>::Output, Canceled>,
        <Self as RuntimeJoinHandle>::Error,
    >,
>
{
    type Output: Send + 'static;
    type Error: Error + Send + 'static;
}

/// A handle for joining on a spawned task.
///
/// This handle is created by awaiting a future returned by [`spawn`] function.
///
/// [`spawn`]: crate::spawn()
#[pin_project]
pub struct JoinHandle<H> {
    #[pin]
    pub(crate) inner: H,
}

impl<T, H> Future for JoinHandle<H>
where
    H: RuntimeJoinHandle<Output = T>,
    T: Send + 'static,
{
    type Output = Result<T, Box<dyn std::error::Error + Send + 'static>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.project().inner.poll(cx) {
            Poll::Ready(Ok(Ok(v))) => Poll::Ready(Ok(v)),
            Poll::Ready(Ok(Err(e))) => Poll::Ready(Err(Box::new(e))),
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
