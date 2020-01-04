use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{Canceled, Cancellation};

#[pin_project]
pub struct Cancelable<T> {
    #[pin]
    inner: T,
    #[pin]
    cancel: Cancellation,
    forced: bool,
}

impl<T> Cancelable<T> {
    pub fn poll_cancel(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Canceled>> {
        let mut this = self.project();
        // if the cancellation is forced, no need for polling again
        if *this.forced {
            return Poll::Ready(Some(Canceled::Forced));
        }

        // check if the scope is canceled
        this.cancel.as_mut().poll(cx).map(|cancel| {
            if let Some(reason) = cancel {
                if reason == Canceled::Forced {
                    *this.forced = true;
                }

                Some(reason)
            } else {
                None
            }
        })
    }
}

// these impls are based on futures' Abortable
impl<T> Future for Cancelable<T>
where
    T: Future,
{
    type Output = Result<T::Output, Canceled>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
            return Poll::Ready(Err(reason));
        }

        self.project().inner.poll(cx).map(Ok)
    }
}

mod tokio_impl {
    use bytes::buf::{Buf, BufMut};
    use std::mem::MaybeUninit;
    use tokio::io::{AsyncRead, AsyncWrite, Error, ErrorKind, Result};

    use super::*;

    impl<T> AsyncRead for Cancelable<T>
    where
        T: AsyncRead,
    {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &mut [u8],
        ) -> Poll<Result<usize>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_read(cx, buf)
        }

        unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
            self.inner.prepare_uninitialized_buffer(buf)
        }

        fn poll_read_buf<B: BufMut>(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &mut B,
        ) -> Poll<Result<usize>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_read_buf(cx, buf)
        }
    }

    // TODO: not sure how cancellation and shutdown interacts.
    // we should probably call poll_shutdown on cancallation.
    impl<T> AsyncWrite for Cancelable<T>
    where
        T: AsyncWrite,
    {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            // TODO: shutdown on graceful cancellation?
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<()>> {
            // TODO: shutdown on graceful cancellation?
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<()>> {
            // TODO: keep shutting down on graceful cancellation?
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_shutdown(cx)
        }

        fn poll_write_buf<B: Buf>(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &mut B,
        ) -> Poll<Result<usize>> {
            // TODO: shutdown on graceful cancellation?
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_cancel(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_write_buf(cx, buf)
        }
    }
}

pub fn cancelable<T>(v: T) -> Cancelable<T> {
    Cancelable {
        inner: v,
        cancel: Cancellation::none(),
        forced: false,
    }
}
