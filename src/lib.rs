use futures_intrusive::channel::shared::StateReceiver;
use futures_intrusive::sync::Semaphore;

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
pub use cancellation::Cancellation;
pub use scope::scope;
pub use spawn::spawn;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Canceled {
    /// The parent task issued a graceful cancellation request.
    /// The parent will wait until all children complete shutdown.
    Graceful,
    /// The parent task is forcibly shutting down children (mainly because the parent is dropped).
    /// The children are in "mercy" mode; They can continue execution until the next cancellation
    /// point, and then they will be dropped.
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

// based on ArcReleaser in
// https://github.com/Matthias247/futures-intrusive/issues/9
pub(crate) struct Join {
    pub(crate) sem: Arc<Semaphore>,
}

impl Drop for Join {
    fn drop(&mut self) {
        self.sem.release(1);
    }
}

#[derive(Clone)]
pub(crate) struct Token {
    // notifies when the scope is canceled (dropped)
    pub(crate) cancel: Arc<StateReceiver<bool>>,
    // notfies when all of the spawned children finish
    pub(crate) join: Weak<Join>,
}
