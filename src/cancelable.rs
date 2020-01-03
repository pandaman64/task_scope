use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::waker;
use crate::Canceled;

#[pin_project]
#[derive(Debug)]
pub struct Cancelable<T>(#[pin] T);

// these impls are based on futures' Abortable
impl<T> Future for Cancelable<T>
where
    T: Future,
{
    type Output = Result<T::Output, Canceled>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // check whether the task is canceled first
        if waker::is_canceled(cx) {
            return Poll::Ready(Err(Canceled));
        }

        match self.project().0.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(v) => Poll::Ready(Ok(v)),
        }
    }
}

pub fn cancelable<T>(v: T) -> Cancelable<T> {
    Cancelable(v)
}
