#![feature(const_waker, type_alias_impl_trait)]

use std::future::Future;
use std::pin::pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use maybe_async_proc_macro::maybe_async;
use maybe_async_std::{prelude::*, sleep};

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
    sleep::<NotAsync>();
    sleep_and_print::<NotAsync>();
}

#[test]
fn async_call() {
    run_to_completion(async {
        sleep::<Async>().await;
        sleep_and_print::<Async>().await;
    });
}

#[maybe_async]
async fn sleep_and_print() {
    let _ = sleep().await;
}
