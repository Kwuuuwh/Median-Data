mod conflict;
mod resolve;
mod source;

pub use conflict::{Conflict, claims};
pub use resolve::{Claim, Resolved, Status, resolve};
pub use source::Source;
