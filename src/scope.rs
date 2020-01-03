use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::future::{maybe_done, MaybeDone};
use futures::never::Never;
use pin_project::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};

use crate::with_cancellation::WithCancellation;

#[derive(Clone)]
pub(crate) struct Cancellation {
    pub(crate) sender: Weak<Mutex<Sender<Never>>>,
}

impl Cancellation {
    pub(crate) fn new(sender: Weak<Mutex<Sender<Never>>>) -> Self {
        Self { sender }
    }
}

#[pin_project]
pub struct ScopeFuture<F>
where
    F: Future,
{
    #[pin]
    inner: WithCancellation<MaybeDone<F>>,
    sender: Option<Arc<Mutex<Sender<Never>>>>,
    #[pin]
    receiver: Receiver<Never>,
}

impl<F> Future for ScopeFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.project();

        match this.inner.inner {
            MaybeDone::Gone => panic!("poll after completion"),
            MaybeDone::Future(_) => {
                let sender = this.sender.as_ref().expect("poll after completion");
                let cancellation = Cancellation {
                    sender: Arc::downgrade(sender),
                };

                if let Poll::Pending = this.inner.as_mut().poll(cx, cancellation) {
                    return Poll::Pending;
                }

                this.sender.take();
            }
            MaybeDone::Done(_) => {}
        }

        match this.receiver.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(_)) => Poll::Ready(this.inner.project().inner.take_output().unwrap()),
            Poll::Ready(Ok(_)) => unreachable!(),
        }
    }
}

pub fn scope<Fut>(inner: Fut) -> ScopeFuture<Fut>
where
    Fut: Future,
{
    let (sender, receiver) = channel();

    ScopeFuture {
        inner: WithCancellation::new(maybe_done(inner)),
        sender: Some(Arc::new(Mutex::new(sender))),
        receiver,
    }
}
