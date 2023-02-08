//! A channel which may or may not be async.
//!
//! This crate is an experimental desugaring of what actual "maybe-async" syntax
//! might look like.
//!
//! # Syntactic Sugar
//!
//! We can imagine this crate would act as a desugaring for the following:
//! ```rust,ignore
//! pub ?async fn bounded(cap: usize) -> (?async Sender<T>, ?async Receiver<T>);
//! pub ?async fn unbounded() -> (?async Sender<T>, ?async Receiver<T>);
//!
//! pub ?async struct Sender<T>;
//! impl<T> ?async Sender<T> {
//!     ?async fn send(&self, msg: T) -> Result<(), SendError>;
//! }
//!
//! pub ?async struct Receiver<T>;
//! impl<T> ?async Receiver<T> {
//!     ?async fn recv(&self) -> Result<T, RecvError>;
//! }
//! ```

#![forbid(unsafe_code, future_incompatible, rust_2018_idioms)]
#![deny(nonstandard_style)]
#![warn(missing_docs, unreachable_pub)]
#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![allow(incomplete_features)]

use std::future::Future;

use sender::SenderDataHelper;

pub(crate) mod bounded {
    use super::*;

    pub(crate) trait BoundedHelper<T, const ASYNC: bool> {
        fn bounded(cap: usize) -> (Sender<T, ASYNC>, Receiver<T, ASYNC>);
    }

    impl<T> BoundedHelper<T, true> for () {
        fn bounded(cap: usize) -> (Sender<T, true>, Receiver<T, true>) {
            let (sender, receiver) = async_channel::bounded(cap);
            let sender = Sender { sender };
            let receiver = Receiver { receiver };
            (sender, receiver)
        }
    }

    impl<T> BoundedHelper<T, false> for () {
        fn bounded(cap: usize) -> (Sender<T, false>, Receiver<T, false>) {
            let (sender, receiver) = crossbeam_channel::bounded(cap);
            let sender = Sender { sender };
            let receiver = Receiver { receiver };
            (sender, receiver)
        }
    }

    // Actually only an impl for `MaybeAsync<false>`, as there are only two possible impls
    // and we wrote both of them. Workaround for https://github.com/rust-lang/rust/pull/104803
    impl<T, const ASYNC: bool> BoundedHelper<T, ASYNC> for () {
        default fn bounded(_cap: usize) -> (Sender<T, ASYNC>, Receiver<T, ASYNC>) {
            panic!("your trait solver is broken")
        }
    }
}

/// Creates a bounded channel.
///
/// The created channel has space to hold at most `cap` messages at a time.
pub fn bounded<T, const ASYNC: bool>(cap: usize) -> (Sender<T, ASYNC>, Receiver<T, ASYNC>) {
    <() as bounded::BoundedHelper<T, ASYNC>>::bounded(cap)
}

/// Creates an unbounded channel.
///
/// The created channel can hold an unlimited number of messages.
pub fn unbounded<T, const ASYNC: bool>() -> (Sender<T, ASYNC>, Receiver<T, ASYNC>) {
    todo!();
}

/// The sending side of a channel.
pub struct Sender<T, const ASYNC: bool> {
    sender: <() as sender::SenderDataHelper<T, ASYNC>>::Data,
}

impl<T, const ASYNC: bool> Sender<T, ASYNC> {
    /// Send an item on the channel
    pub fn send(&mut self, t: T) -> <() as sender::SenderDataHelper<T, ASYNC>>::Ret<'_> {
        <() as SenderDataHelper<T, ASYNC>>::send(self, t)
    }
}

mod sender {
    use super::*;
    /// Support trait for `Sender`.
    pub trait SenderDataHelper<T, const ASYNC: bool> {
        /// What is the type we're returning?
        type Data;
        /// What is the type `send` is returning
        type Ret<'a>
        where
            Self: 'a,
            T: 'a;
        fn send(sender: &mut Sender<T, ASYNC>, _: T) -> Self::Ret<'_>;
    }

    impl<T> SenderDataHelper<T, true> for () {
        type Data = async_channel::Sender<T>;
        type Ret<'a> = impl std::future::Future<Output = Result<(), async_channel::SendError<T>>> + 'a where Self: 'a, T: 'a;
        fn send(sender: &mut Sender<T, true>, msg: T) -> Self::Ret<'_> {
            sender.sender.send(msg)
        }
    }

    impl<T> SenderDataHelper<T, false> for () {
        type Data = crossbeam_channel::Sender<T>;
        type Ret<'a> = Result<(), crossbeam_channel::SendError<T>> where Self: 'a, T: 'a;
        fn send(sender: &mut Sender<T, false>, msg: T) -> Self::Ret<'_> {
            sender.sender.send(msg)
        }
    }

    // Actually only an impl for `MaybeAsync<false>`, as there are only two possible impls
    // and we wrote both of them. Workaround for https://github.com/rust-lang/rust/pull/104803
    impl<T, const B: bool> SenderDataHelper<T, B> for () {
        default type Data = ();
        default type Ret<'a> = () where Self: 'a, T: 'a;
        default fn send(_sender: &mut Sender<T, B>, _msg: T) -> Self::Ret<'_> {
            panic!("your trait solver is broken")
        }
    }
}

/// The Receiving side of a channel.
pub struct Receiver<T, const ASYNC: bool> {
    receiver: <Self as receiver::ReceiverDataHelper<ASYNC>>::Data,
}

pub(crate) mod receiver {
    use super::*;

    /// Support trait for `Sender`.
    pub(crate) trait ReceiverDataHelper<const ASYNC: bool> {
        /// What is the type we're returning?
        type Data;
    }

    impl<T> ReceiverDataHelper<true> for Receiver<T, true> {
        type Data = async_channel::Receiver<T>;
    }

    impl<T> ReceiverDataHelper<false> for Receiver<T, false> {
        type Data = crossbeam_channel::Receiver<T>;
    }

    // Actually only an impl for `MaybeAsync<false>`, as there are only two possible impls
    // and we wrote both of them. Workaround for https://github.com/rust-lang/rust/pull/104803
    impl<T, const ASYNC: bool> ReceiverDataHelper<ASYNC> for Receiver<T, ASYNC> {
        default type Data = ();
    }
}

/// An interface for dealing with iterators.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub trait Iterator<const ASYNC: bool> {
    type Item;
    type MaybeFuture<'a>
    where
        Self: 'a;
    fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a>;
}

// Actually only an impl for `MaybeAsync<false>`, as there are only two possible impls
// and we wrote both of them. Workaround for https://github.com/rust-lang/rust/pull/104803
impl<T, const ASYNC: bool> Iterator<ASYNC> for Receiver<T, ASYNC> {
    default type Item = ();
    default type MaybeFuture<'a> = ()
    where
        Self: 'a;
    default fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a> {
        panic!("your trait solver is broken")
    }
}

impl<T> Iterator<false> for Receiver<T, false> {
    type Item = T;
    type MaybeFuture<'a> = Option<T>
    where
        Self: 'a;
    fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a> {
        self.receiver.recv().ok()
    }
}

impl<T> Iterator<true> for Receiver<T, true> {
    type Item = T;
    type MaybeFuture<'a> = impl Future<Output = Option<T>> + 'a
    where
        Self: 'a;
    fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a> {
        async move { self.receiver.recv().await.ok() }
    }
}
