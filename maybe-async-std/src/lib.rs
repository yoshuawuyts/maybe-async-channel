#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![feature(associated_type_defaults)]
#![feature(async_iterator)]
#![allow(incomplete_features)]

use std::{async_iter::AsyncIterator, future::Future};

use maybe_async_proc_macro::maybe;

mod maybe_async_std {
    pub use super::*;
}

#[maybe(async)]
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

#[maybe(async)]
pub trait Iterator {
    type Item;
    #[maybe(async)]
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

struct OptionIter<T>(Option<T>);

impl<T: Future> Iterator<true> for OptionIter<T> {
    type Item = <T as Future>::Output;
    type next_ret<'a> = impl Future<Output = Option<<T as Future>::Output>> + 'a where T: 'a;

    fn next<'a>(&'a mut self) -> Self::next_ret<'a> {
        async move {
            match self.0.take() {
                Some(val) => Some(val.await),
                None => None,
            }
        }
    }
}

impl<I: AsyncIterator> Iterator<true> for I {
    type Item = <I as AsyncIterator>::Item;
    type next_ret<'a> = impl Future<Output = Option<Self::Item>> + 'a where I: 'a;

    fn next<'a>(&'a mut self) -> Self::next_ret<'a> {
        Fut(self)
    }
}

struct Fut<'a, T: AsyncIterator>(&'a mut T);

impl<'a, I: AsyncIterator> Unpin for Fut<'a, I> {}

impl<'a, T: AsyncIterator> Future for Fut<'a, T> {
    type Output = Option<<T as AsyncIterator>::Item>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let pinned = unsafe { self.map_unchecked_mut(|this| this.0) };
        AsyncIterator::poll_next(pinned, cx)
    }
}
