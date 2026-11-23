mod secret;
mod timed;

pub use secret::Secret;
pub use timed::{Pending, TimedError, timed};
