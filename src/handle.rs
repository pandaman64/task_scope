use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::Canceled;

#[pin_project]
pub struct JoinHandle<T> {
    #[pin]
    pub(crate) inner: tokio::task::JoinHandle<Result<T, Canceled>>,
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, Box<dyn std::error::Error + 'static>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.project().inner.poll(cx) {
            Poll::Ready(Ok(Ok(v))) => Poll::Ready(Ok(v)),
            Poll::Ready(Ok(Err(e))) => Poll::Ready(Err(Box::new(e))),
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
