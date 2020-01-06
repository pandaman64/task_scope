use futures::future::FusedFuture;
use futures::stream::{FusedStream, Stream};
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{Canceled, Cancellation};

/// A cancelable wrapper for asynchronous operations.
///
/// This wrapper is created by [`cancelable`] function.
#[must_use = "do nothing unless polled"]
#[pin_project]
pub struct Cancelable<T> {
    #[pin]
    inner: T,
    #[pin]
    cancel: Cancellation,
    forced: bool,
}

impl<T> Cancelable<T> {
    /// Polls whether the task is canceled.
    ///
    /// # Returns
    /// Returns `Ready(Some(reason))` if the task is canceled.
    /// Returns `Ready(None)` if the context doesn't support cancellation.
    pub fn poll_canceled(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Canceled>> {
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

    /// retrieve the inner value.
    pub fn inner(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner
    }
}

impl<T> Future for Cancelable<T>
where
    T: Future,
{
    type Output = Result<T::Output, Canceled>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
            return Poll::Ready(Err(reason));
        }

        self.project().inner.poll(cx).map(Ok)
    }
}

impl<T> FusedFuture for Cancelable<T>
where
    T: FusedFuture,
{
    fn is_terminated(&self) -> bool {
        self.forced || self.inner.is_terminated()
    }
}

impl<T> Stream for Cancelable<T>
where
    T: Stream,
{
    type Item = Result<T::Item, Canceled>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
            return Poll::Ready(Some(Err(reason)));
        }

        self.project().inner.poll_next(cx).map(|x| x.map(Ok))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> FusedStream for Cancelable<T>
where
    T: FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.forced || self.inner.is_terminated()
    }
}

mod futures_impl {
    use futures::io::{AsyncRead, AsyncWrite, Error, ErrorKind, Result};

    use super::*;

    // TODO: figure out how vectored I/O and initializer work
    impl<T> AsyncRead for Cancelable<T>
    where
        T: AsyncRead,
    {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &mut [u8],
        ) -> Poll<Result<usize>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_read(cx, buf)
        }
    }

    impl<T> AsyncWrite for Cancelable<T>
    where
        T: AsyncWrite,
    {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<()>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_flush(cx)
        }

        fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<()>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_close(cx)
        }
    }
}

#[cfg(feature = "tokio")]
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
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
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
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_read_buf(cx, buf)
        }
    }

    impl<T> AsyncWrite for Cancelable<T>
    where
        T: AsyncWrite,
    {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<()>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<()>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_shutdown(cx)
        }

        fn poll_write_buf<B: Buf>(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &mut B,
        ) -> Poll<Result<usize>> {
            if let Poll::Ready(Some(reason)) = self.as_mut().poll_canceled(cx) {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, reason)));
            }

            self.project().inner.poll_write_buf(cx, buf)
        }
    }
}

/// Creates a cancelable wrapper for asynchronous operations (`Future`/`AsyncRead`/`AsyncWrite`).
///
/// When the returned wrapper is polled, it will check if the task is canceled first. If the task is
/// canceled (regardless of whether the cancellation is graceful or forced), the wrapper will not
/// poll the inner future again and return an error.
///
/// This function can be used to introduce a cancellation point in a task.
pub fn cancelable<T>(v: T) -> Cancelable<T> {
    Cancelable {
        inner: v,
        cancel: Cancellation::none(),
        forced: false,
    }
}
