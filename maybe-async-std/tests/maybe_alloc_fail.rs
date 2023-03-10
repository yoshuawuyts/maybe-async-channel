#![feature(const_waker, type_alias_impl_trait)]
#![feature(specialization)]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(adt_const_params)]
#![feature(allocator_api)]
#![allow(incomplete_features)]

use maybe_async_proc_macro::maybe;
use maybe_async_std::mk_box;
use maybe_async_std::prelude::*;

#[test]
fn host_call() {
    mk_box::<{ Effects::NONE }>();
    mk_box_and_print::<{ Effects::NONE }>();
}

#[test]
fn try_call() {
    fn foomp() -> Result<(), std::alloc::AllocError> {
        mk_box::<{ Effects::TRY }>()?;
        mk_box_and_print::<{ Effects::TRY }>()?;
        Ok(())
    }
    foomp().unwrap();
}

#[maybe(try)]
fn mk_box_and_print() -> Result<(), std::alloc::AllocError> {
    let _ = mk_box()?;
}
