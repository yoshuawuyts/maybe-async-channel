#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![allow(incomplete_features)]

use std::future::Future;

use maybe_async_proc_macro::maybe_async;

mod maybe_async_std {
    pub use super::*;
}

#[maybe_async]
pub fn sleep(dur: std::time::Duration) {
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
    #[maybe_async]
    fn next(&mut self) -> Option<Self::Item>;
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<T> Iterator for Option<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.take()
    }
}

impl<T: Future> Iterator<true> for Option<T> {
    type Item = <T as Future>::Output;
    type next_ret<'a> = impl Future<Output = Option<<T as Future>::Output>> + 'a where T: 'a;

    fn next<'a>(&'a mut self) -> Self::next_ret<'a> {
        async move {
            match self.take() {
                Some(val) => Some(val.await),
                None => None,
            }
        }
    }
}
