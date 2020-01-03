use futures::never::Never;
use futures_intrusive::channel::shared::ChannelReceiveFuture;
use pin_project::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[pin_project]
pub(crate) struct CancellationFuture {
    #[pin]
    inner: ChannelReceiveFuture<parking_lot::RawMutex, Never>,
}

impl CancellationFuture {
    pub(crate) fn new(inner: ChannelReceiveFuture<parking_lot::RawMutex, Never>) -> Self {
        Self { inner }
    }
}

impl Future for CancellationFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.project()
            .inner
            .poll(cx)
            .map(|ret| assert!(ret.is_none()))
    }
}
