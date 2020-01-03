#![feature(weak_counts)]

use std::fmt;

pub mod cancelable;
pub mod handle;
pub mod scope;
pub mod spawn;
mod waker;
mod with_cancellation;

#[derive(Debug)]
pub struct Canceled;

impl fmt::Display for Canceled {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "task is canceled")
    }
}

impl std::error::Error for Canceled {}

pub use cancelable::cancelable;
pub use scope::scope;
pub use spawn::spawn;
