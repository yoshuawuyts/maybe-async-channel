#![feature(pin_macro, const_waker)]

use maybe_async_channel::*;
use std::future::Future;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::pin::pin;

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
    let (mut sender, _receiver) = bounded::<helpers::NotAsync, usize>(10);
    run_to_completion(sender.send(42));
    sender.send2(42);
}

#[test]
fn async_call() {
    let _ = async {
        let (mut sender, _receiver) = bounded::<helpers::Async, usize>(42);
        sender.send(42).await;
        sender.send2(42).await;
    };
}
