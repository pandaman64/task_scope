//! A runtime extension for adding support for Structured Concurrency to existing runtimes.
//!
//! # What is Strucutured Concurrency?
//! [Structured Concurrency] is a programming paradigm that lets asynchronous operations run only
//! within certain scopes so that they form an operation stack like a regular function call stack.
//! As the parent operation waits until all children complete, Structured Concurrency helps local
//! reasoning of concurrent programs.
//!
//! # Out-of-task concurrency considered harmful
//! Most of the asynchronous programs are composed of `async` functions and *in-task* concurrency
//! primitives such as `select` and `join`, and this makes such futures automatically
//! well-structured. As "futures do nothing unless polled," executions of inner (child) operations
//! are very explicit (usually at the point of `await`s). Moreover, canceling a future is done by
//! dropping it, which will reclaim resources used for the operation, including the inner futures.
//! This drop-chain propagates the cancellation to the very end of the operation stack.
//!
//! *Out-of-task* concurrencies such as `spawn`, however, breaks the structure. They allow us to
//! start a new execution unit that can escape from the parent stack. Although frameworks provide a
//! way to join on the spawned task, they don't propagate cancellation properly. If you drop the
//! spawning task, the spawned task may outlive the parent indefinitely.
//!
//! `task_scope` is designed to offer a [`spawn`] function that properly respects cancellation.
//! [`scope`] function delimits a lifetime within which inner tasks are allowed to run. If you
//! issue a graceful cancellation or drop the scope, the runtime delivers a cancellation signal to all
//! the child tasks.
//!
//! # Cancellation points
//! `task_scope` requires the tasks to pass through cancellation points regularly to work effectively.
//! Consider the following (contrived) example:
//!
//! ```no_run
//! use tokio::io::*;
//!
//! # async fn doc_loop() {
//! let mut read = repeat(42); // very fast input
//! let mut write = sink(); // very fast output
//!
//! copy(&mut read, &mut write).await.unwrap();
//! # }
//! ```
//!
//! This program is virtually running into an infinite loop because `read` and `write` never
//! terminate. To make matters worse, this loop cannot be canceled from the outside because the
//! I/O operations always succeed, and `copy` function tries to continue as much as possible.
//! Therefore, the spawned task must cooperatively check for cancellation or yield the execution
//! regularly in a loop to preserve the well-structuredness.
//!
//! `task_scope` provides a convenience function [`cancelable`] to handle cancellation
//! automatically. It wraps the given `Future`/`AsyncRead`/`AsyncWrite` and checks for cancellation
//! (graceful or forced) before proceeding with the inner computation. The example above will look
//! like:
//!
//! ```no_run
//! use futures::pin_mut;
//! use tokio::io::*;
//!
//! use task_scope::cancelable;
//!
//! # async fn doc_cancelable_loop() {
//! let read = cancelable(repeat(42)); // very fast, but cancelable input
//! pin_mut!(read); // needed for Unpin bound of copy
//! let mut write = sink(); // very fast output
//!
//! // this will terminate with an error on cancellation
//! copy(&mut read, &mut write).await.unwrap();
//! # }
//! ```
//!
//! If the cancellation logic is more complex, you can poll [`Cancellation`] manually to check for
//! a cancellation signal.
//!
//! # Grace period and mercy period
//! `task_scope` supports two modes of cancellation. Graceful cancellation and forced cancellation.
//!
//! You can initiate a graceful cancellation by calling [`cancel`] method of the scope. This
//! notifies and gives a "grace period" to tasks in the scope so that they can start their
//! cancellation. The scope waits for all the tasks to complete as usual.
//!
//! A forced cancellation is propagated to tasks when the scope is dropped, or [`force_cancel`] is
//! called. Given that canceling a long-running is really hard as shown above, the canceled tasks
//! are given to a "mercy period." The tasks can continue their execution until they yield the
//! execution next time, and then the runtime will automatically cancel the task. The tasks should
//! shorten the mercy period as short as possible because it's technically breaking the program's
//! concurrency structure (a child is outliving the dropped parent).
//!
//! [Structured Concurrency]: https://en.wikipedia.org/wiki/Structured_concurrency
//! [`spawn`]: crate::spawn()
//! [`scope`]: crate::scope()
//! [`cancelable`]: crate::cancelable()
//! [`Cancellation`]: crate::Cancellation
//! [`cancel`]: crate::scope::ScopeFuture::cancel
//! [`force_cancel`]: crate::scope::ScopeFuture::force_cancel
//!

use futures_intrusive::channel::shared::StateReceiver;
use futures_intrusive::sync::ManualResetEvent;

use std::fmt;
use std::sync::{Arc, Weak};

pub mod cancelable;
pub mod cancellation;
pub mod handle;
pub mod scope;
pub mod spawn;
mod waker;
mod with_token;

pub use cancelable::{cancelable, Cancelable};
pub use cancellation::{cancellation, Cancellation};
pub use scope::scope;
pub use spawn::*;

/// The error type for cancellation.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Canceled {
    /// The parent task issued a graceful cancellation request.
    /// The children are in a "grace period"; The parent will wait until all children complete shutdown.
    Graceful,
    /// The parent task is forcibly shutting down children (mainly because the parent is dropped).
    /// The children are in a "mercy period"; They can continue execution until the next yield,
    /// and then they will be stopped automatically.
    Forced,
}

impl fmt::Display for Canceled {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Canceled::Graceful => write!(f, "graceful cancellation is requested"),
            Canceled::Forced => write!(f, "forced cancellation is requested"),
        }
    }
}

impl std::error::Error for Canceled {}

pub(crate) struct Join {
    pub(crate) event: Arc<ManualResetEvent>,
}

impl Drop for Join {
    fn drop(&mut self) {
        self.event.set();
    }
}

#[derive(Clone)]
pub(crate) struct Token {
    // notifies when the scope is canceled (dropped)
    pub(crate) cancel: Arc<StateReceiver<bool>>,
    // notfies when all of the spawned children finish
    pub(crate) join: Weak<Join>,
}
