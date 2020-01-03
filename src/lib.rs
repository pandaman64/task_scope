use futures::never::Never;
use futures_intrusive::channel::shared::OneshotReceiver;
use futures_intrusive::sync::Semaphore;

use std::fmt;
use std::sync::{Arc, Weak};

pub mod cancelable;
mod cancellation_future;
pub mod handle;
pub mod scope;
pub mod spawn;
mod waker;
mod with_token;

pub use cancelable::cancelable;
pub use scope::scope;
pub use spawn::spawn;

#[derive(Debug)]
pub struct Canceled;

impl fmt::Display for Canceled {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "task is canceled")
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
    pub(crate) cancel: Arc<OneshotReceiver<Never>>,
    // notfies when all of the spawned children finish
    pub(crate) join: Weak<Join>,
}
