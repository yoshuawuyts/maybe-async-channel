#![feature(type_alias_impl_trait)]

use maybe_async_proc_macro::maybe_async;

/// Mark a type to be compiled in "async mode"
#[derive(Debug)]
pub struct Async;

/// Mark a type to be compiled in "!async mode"
#[derive(Debug)]
pub struct NotAsync;

pub mod prelude {
    pub use super::{Async, NotAsync};
}

mod maybe_async_std {
    pub use super::*;
}

#[maybe_async]
pub async fn sleep() {}
