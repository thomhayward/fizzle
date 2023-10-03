mod client;
pub mod query;
pub mod util;
pub mod write;

pub use write::precision::Precision;

pub use client::Client;

pub use write::buffered;
pub use write::immediate;
pub use write::LineBuilder;
pub use write::Status;
