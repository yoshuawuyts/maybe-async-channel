//! A channel which may or may not be async
//!
//! # Examples
//!
//! ```
//! // tbi
//! ```

#![forbid(unsafe_code, future_incompatible, rust_2018_idioms)]
#![deny(nonstandard_style)]
#![warn(missing_docs, missing_doc_code_examples, unreachable_pub)]
#![feature(type_alias_impl_trait)]

/// A crate of helpers to create "maybe-async" types and traits.
///
/// When we have compiler support for "maybe-async" this will be emitted by the
/// desugaring, and should not surface to users of the feature.
pub mod helpers {
    /// A bound on types which determines whether a type is async or not.
    pub trait MaybeAsync: Sized {}

    impl MaybeAsync for NotAsync {}

    impl MaybeAsync for Async {}

    /// Mark a type to be compiled in "!async mode"
    #[derive(Debug)]
    pub struct NotAsync;

    /// Mark a type to be compiled in "async mode"
    #[derive(Debug)]
    pub struct Async;
}

use helpers::*;
use sender::SenderDataHelper;

pub trait BoundedHelper<T>: MaybeAsync {
    fn bounded(cap: usize) -> (Sender<Self, T>, Receiver<Self, T>)
    where
        Sender<Self, T>: sender::SenderDataHelper<T>,
        Receiver<Self, T>: receiver::ReceiverDataHelper;
}

impl<T> BoundedHelper<T> for Async {
    fn bounded(cap: usize) -> (Sender<Self, T>, Receiver<Self, T>) {
        let (sender, receiver) = async_channel::bounded(cap);
        let sender = Sender { sender };
        let receiver = Receiver { receiver };
        (sender, receiver)
    }
}

impl<T> BoundedHelper<T> for NotAsync {
    fn bounded(cap: usize) -> (Sender<Self, T>, Receiver<Self, T>) {
        let (sender, receiver) = crossbeam_channel::bounded(cap);
        let sender = Sender { sender };
        let receiver = Receiver { receiver };
        (sender, receiver)
    }
}

/// Creates a bounded channel.
///
/// The created channel has space to hold at most `cap` messages at a time.
pub fn bounded<E: BoundedHelper<T>, T>(cap: usize) -> (Sender<E, T>, Receiver<E, T>)
where
    Sender<E, T>: sender::SenderDataHelper<T>,
    Receiver<E, T>: receiver::ReceiverDataHelper,
{
    E::bounded(cap)
}

/// Creates an unbounded channel.
///
/// The created channel can hold an unlimited number of messages.
pub fn unbounded<E: MaybeAsync, T>() -> (Sender<E, T>, Receiver<E, T>)
where
    Sender<E, T>: sender::SenderDataHelper<T>,
    Receiver<E, T>: receiver::ReceiverDataHelper,
{
    todo!();
}

/// The sending side of a channel.
pub struct Sender<E: MaybeAsync, T>
where
    Sender<E, T>: sender::SenderDataHelper<T>,
{
    sender: <Self as sender::SenderDataHelper<T>>::Data,
}

impl<E: MaybeAsync, T> Sender<E, T>
where
    Self: sender::SenderDataHelper<T>,
{
    pub async fn send(&mut self, _: T) {}
    pub fn send2(&mut self, t: T) -> <Self as sender::SenderDataHelper<T>>::Ret {
        <Self as SenderDataHelper<T>>::send(self, t)
    }
}

mod sender {
    use super::*;
    /// Support trait for `Sender`.
    pub trait SenderDataHelper<T> {
        /// What is the type we're returning?
        type Data;
        /// What is the type `send` is returning
        type Ret;
        fn send(&mut self, _: T) -> Self::Ret;
    }

    impl<T> SenderDataHelper<T> for Sender<Async, T> {
        type Data = async_channel::Sender<T>;
        type Ret = impl std::future::Future<Output = ()>;
        fn send(&mut self, _: T) -> Self::Ret {
            async {}
        }
    }
    impl<T> SenderDataHelper<T> for Sender<NotAsync, T> {
        type Data = crossbeam_channel::Sender<T>;
        type Ret = ();
        fn send(&mut self, _: T) {}
    }
}

/// The Receiving side of a channel.
pub struct Receiver<E: MaybeAsync, T>
where
    Receiver<E, T>: receiver::ReceiverDataHelper,
{
    receiver: <Self as receiver::ReceiverDataHelper>::Data,
}

pub(crate) mod receiver {
    use super::*;

    /// Support trait for `Sender`.
    pub trait ReceiverDataHelper {
        /// What is the type we're returning?
        type Data;
    }

    impl<T> ReceiverDataHelper for Receiver<Async, T> {
        type Data = async_channel::Receiver<T>;
    }

    impl<T> ReceiverDataHelper for Receiver<NotAsync, T> {
        type Data = crossbeam_channel::Receiver<T>;
    }
}
