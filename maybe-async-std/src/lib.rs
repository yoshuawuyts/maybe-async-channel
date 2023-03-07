#![feature(type_alias_impl_trait)]

use std::future::Future;

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
pub async fn sleep(dur: std::time::Duration) {
    if ASYNC {
        Sleepy(std::time::Instant::now() + dur)
    } else {
        std::thread::sleep(dur)
    }
}

struct Sleepy(std::time::Instant);

impl Future for Sleepy {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.0 > std::time::Instant::now() {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    }
}

#[maybe_async]
pub trait Iterator {
    type Item;
    async fn next(&mut self) -> Option<Item>;
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<T> Iterator<NotAsync> for Option<T> {
    type Item = T;
    type next_ret<'a> = Option<T> where Self: 'a;

    fn next<'b>(&'b mut self) -> Option<T> {
        self.take()
    }
}
