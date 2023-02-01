#![feature(const_waker, type_alias_impl_trait)]

use maybe_async_proc_macro::maybe_async;
use std::future::Future;
use std::pin::pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[maybe_async]
async fn sleep() {}

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
    sleep::<sleep::NotAsync>();
}

#[test]
fn async_call() {
    run_to_completion(async {
        sleep::<sleep::Async>().await;
    });
}
