#![feature(const_waker)]

use maybe_async_channel::*;
use std::future::Future;
use std::pin::pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

fn run_to_completion<T>(f: impl Future<Output = T>) -> T {
    const WAKER: &Waker = {
        const RAW: RawWaker = {
            RawWaker::new(
                ptr::null(),
                &RawWakerVTable::new(no_clone, no_wake, no_wake, no_drop),
            )
        };
        fn no_clone(_: *const ()) -> RawWaker {
            RAW
        }
        fn no_wake(_: *const ()) {}
        fn no_drop(_: *const ()) {}
        &unsafe { Waker::from_raw(RAW) }
    };
    let mut ctx = Context::from_waker(&WAKER);
    match pin!(f).poll(&mut ctx) {
        Poll::Ready(res) => res,
        Poll::Pending => unreachable!(),
    }
}

#[test]
fn sync_call() {
    let (mut sender, _receiver) = bounded::<usize, false>(10);
    sender.send(42).unwrap();
}

#[test]
fn async_call() {
    run_to_completion(async {
        let (mut sender, _receiver) = bounded::<usize, true>(42);
        sender.send(42).await.unwrap();
    });
}

#[test]
fn sync_recv() {
    let (mut sender, mut receiver) = bounded::<usize, false>(10);
    sender.send(42).unwrap();
    drop(sender);
    while let Some(elem) = receiver.next() {
        assert_eq!(elem, 42);
    }
}

#[test]
fn async_recv() {
    run_to_completion(async {
        let (mut sender, mut receiver) = bounded::<usize, true>(42);
        sender.send(42).await.unwrap();
        drop(sender);
        while let Some(elem) = receiver.next().await {
            assert_eq!(elem, 42);
        }
    });
}
