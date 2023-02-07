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

use std::future::Future;

use helpers::*;
use sender::SenderDataHelper;

pub(crate) mod bounded {
    use super::*;

    pub trait BoundedHelper<T>: MaybeAsync {
        fn bounded(cap: usize) -> (Sender<T, Self>, Receiver<T, Self>)
        where
            Sender<T, Self>: sender::SenderDataHelper<T>,
            Receiver<T, Self>: receiver::ReceiverDataHelper;
    }

    impl<T> BoundedHelper<T> for Async {
        fn bounded(cap: usize) -> (Sender<T, Self>, Receiver<T, Self>) {
            let (sender, receiver) = async_channel::bounded(cap);
            let sender = Sender { sender };
            let receiver = Receiver { receiver };
            (sender, receiver)
        }
    }

    impl<T> BoundedHelper<T> for NotAsync {
        fn bounded(cap: usize) -> (Sender<T, Self>, Receiver<T, Self>) {
            let (sender, receiver) = crossbeam_channel::bounded(cap);
            let sender = Sender { sender };
            let receiver = Receiver { receiver };
            (sender, receiver)
        }
    }
}

/// Creates a bounded channel.
///
/// The created channel has space to hold at most `cap` messages at a time.
pub fn bounded<T, E: bounded::BoundedHelper<T>>(cap: usize) -> (Sender<T, E>, Receiver<T, E>)
where
    Sender<T, E>: sender::SenderDataHelper<T>,
    Receiver<T, E>: receiver::ReceiverDataHelper,
{
    E::bounded(cap)
}

/// Creates an unbounded channel.
///
/// The created channel can hold an unlimited number of messages.
pub fn unbounded<T, E: MaybeAsync>() -> (Sender<T, E>, Receiver<T, E>)
where
    Sender<T, E>: sender::SenderDataHelper<T>,
    Receiver<T, E>: receiver::ReceiverDataHelper,
{
    todo!();
}

/// The sending side of a channel.
pub struct Sender<T, E: MaybeAsync = NotAsync>
where
    Sender<T, E>: sender::SenderDataHelper<T>,
{
    sender: <Self as sender::SenderDataHelper<T>>::Data,
}

impl<E: MaybeAsync, T> Sender<T, E>
where
    Self: sender::SenderDataHelper<T>,
{
    /// Send an item on the channel
    pub fn send(&mut self, t: T) -> <Self as sender::SenderDataHelper<T>>::Ret<'_> {
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
        type Ret<'a>
        where
            Self: 'a;
        fn send(&mut self, _: T) -> Self::Ret<'_>;
    }

    impl<T> SenderDataHelper<T> for Sender<T, Async> {
        type Data = async_channel::Sender<T>;
        type Ret<'a> = impl std::future::Future<Output = Result<(), async_channel::SendError<T>>> + 'a where Self: 'a;
        fn send(&mut self, msg: T) -> Self::Ret<'_> {
            self.sender.send(msg)
        }
    }
    impl<T> SenderDataHelper<T> for Sender<T, NotAsync> {
        type Data = crossbeam_channel::Sender<T>;
        type Ret<'a> = Result<(), crossbeam_channel::SendError<T>> where Self: 'a;
        fn send(&mut self, msg: T) -> Self::Ret<'_> {
            self.sender.send(msg)
        }
    }
}

/// The Receiving side of a channel.
pub struct Receiver<T, E: MaybeAsync = NotAsync>
where
    Receiver<T, E>: receiver::ReceiverDataHelper,
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

    impl<T> ReceiverDataHelper for Receiver<T, Async> {
        type Data = async_channel::Receiver<T>;
    }

    impl<T> ReceiverDataHelper for Receiver<T, NotAsync> {
        type Data = crossbeam_channel::Receiver<T>;
    }
}

/// An interface for dealing with iterators.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub trait Iterator<ASYNC: MaybeAsync = NotAsync> {
    type Item;
    type MaybeFuture<'a>
    where
        Self: 'a;
    fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a>;
}

impl<T> Iterator<NotAsync> for Receiver<T, NotAsync> {
    type Item = T;
    type MaybeFuture<'a> = Option<T>
    where
        Self: 'a;
    fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a> {
        self.receiver.recv().ok()
    }
}

impl<T> Iterator<Async> for Receiver<T, Async> {
    type Item = T;
    type MaybeFuture<'a> = impl Future<Output = Option<T>> + 'a
    where
        Self: 'a;
    fn next<'a>(&'a mut self) -> Self::MaybeFuture<'a> {
        async move { self.receiver.recv().await.ok() }
    }
}
