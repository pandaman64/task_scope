use futures::future::FusedFuture;
use futures_intrusive::channel::shared::{state_broadcast_channel, StateReceiver, StateSender};
use futures_intrusive::sync::ManualResetEvent;
use pin_project::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::with_token::WithToken;
use crate::{cancellation, Canceled, Cancellation, Join, Token};

fn make_join() -> (Arc<Join>, Arc<ManualResetEvent>) {
    let event = Arc::new(ManualResetEvent::new(false));

    (
        Arc::new(Join {
            event: event.clone(),
        }),
        event,
    )
}

enum State<Fut>
where
    Fut: Future,
{
    Running {
        cancel: Arc<StateReceiver<bool>>,
        join: Arc<Join>,
    },
    Joining(Fut::Output),
    Done,
}

/// A future that manages a scope.
///
/// This future is created by [`scope`] function.
#[must_use = "futures do nothing unless polled"]
#[pin_project]
pub struct ScopeFuture<Fut, Joiner>
where
    Fut: Future,
{
    state: State<Fut>,
    #[pin]
    cancellation: Cancellation,
    #[pin]
    inner: WithToken<Fut>,
    #[pin]
    joiner: Joiner,
    cancel_sender: Option<StateSender<bool>>,
}

impl<Fut, Joiner> ScopeFuture<Fut, Joiner>
where
    Fut: Future,
{
    /// Initiates a graceful cancellation.
    ///
    /// This function will send a wakeup signal to all the
    /// cancellation futures in the scope. Tasks in the scope can continue execution, but they are
    /// expected to stop the subsequent processing during the grace period.
    ///
    /// The cancellation signal will be automatically delivered to all scopes inside `self`.
    pub fn cancel(self: Pin<&mut Self>) {
        // send a cancellation signal
        if let Some(cancel_sender) = &self.cancel_sender {
            let _ = cancel_sender.send(true);
        }
    }

    /// Cancels all the tasks in the scope forcibly.
    /// Tasks within the scope enter a mercy period. They can do the last cleanup before
    /// exitting but will be automatically stopped at the next yield.
    ///
    /// Tasks should shorten the mercy period as short as possible by introducing a cancellation
    /// point regularly, because it's a kind of violation of Structured Concurrency
    /// (children are outliving the dropped parent).
    pub fn force_cancel(self: Pin<&mut Self>) {
        self.project().cancel_sender.take();
    }
}

impl<Fut, Joiner> Future for ScopeFuture<Fut, Joiner>
where
    Fut: Future,
    Joiner: Future<Output = ()>,
{
    type Output = Result<Fut::Output, Canceled>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // check if the parent of this scope (the outer scope) has issued a cancellation.
        // if the cancellation is graceful, we start cancelling this scope. the tasks in the scope
        // keep running, but receive cancellation notification.
        // if the cancellation is forced, we stop executing the scope immediately.
        //
        // TODO: add a API for automatically canceling the main task on a graceful cancellation
        let this = self.as_mut().project();
        match this.cancellation.poll(cx) {
            Poll::Ready(Some(Canceled::Graceful)) => self.as_mut().cancel(),
            Poll::Ready(Some(Canceled::Forced)) => {
                let ret = match std::mem::replace(this.state, State::Done) {
                    State::Joining(v) => Poll::Ready(Ok(v)),
                    _ => Poll::Ready(Err(Canceled::Forced)),
                };

                self.force_cancel();

                return ret;
            }
            _ => {}
        }

        let this = self.project();

        match this.state {
            State::Done => panic!("poll after completion or forced cancellation"),
            State::Running { cancel, join } => {
                let token = Token {
                    cancel: cancel.clone(),
                    join: Arc::downgrade(join),
                };

                match this.inner.poll(cx, token) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => {
                        *this.state = State::Joining(v);
                    }
                }
            }
            _ => {}
        }

        // the main task has finished or been canceled.
        // we'll wait until all spawned children finish execution/cancellation.
        match this.joiner.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => match std::mem::replace(this.state, State::Done) {
                State::Joining(v) => Poll::Ready(Ok(v)),
                _ => unreachable!(),
            },
        }
    }
}

impl<Fut, Joiner> FusedFuture for ScopeFuture<Fut, Joiner>
where
    Fut: Future,
    Joiner: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        match self.state {
            State::Done => true,
            _ => false,
        }
    }
}

/// Create a new task scope.
///
/// The task scope runs the given future as its main task. You can [`spawn`] subtasks within a
/// scope, and the scope will wait until all tasks complete or it is forcibly canceled.
/// If a scope is created within another scope, it will inherit the cancellation signal from the
/// parent. Thus, when the parent is canceled, the child scope is also canceled.
///
/// [`spawn`]: crate::spawn()
///
/// # Returns
/// The returned future will resolve to:
/// - `Ok(v)` when the main task successfully finishes before a forced cancellation.
/// - `Err(e)` when the main task is canceled forcibly.
pub fn scope<Fut>(
    inner: Fut,
) -> ScopeFuture<impl Future<Output = Fut::Output>, impl Future<Output = ()>>
where
    Fut: Future,
{
    // signals receivers when this future is canceled (dropped)
    let (cancel_sender, receiver) = state_broadcast_channel();
    let (join, event) = make_join();

    let joiner = async move {
        event.wait().await;
    };

    ScopeFuture {
        state: State::Running {
            cancel: Arc::new(receiver),
            join,
        },
        cancellation: cancellation(),
        inner: WithToken::new(inner),
        joiner,
        cancel_sender: Some(cancel_sender),
    }
}
