use std::future::Future;

use crate::context::Context;

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod builder;
pub mod service;
pub mod spawner;

pub(crate) mod restart_strategy;
pub use restart_strategy::RestartableActor;

pub type DynResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// An actor is an object that can receive messages.
pub trait Actor: Sized + Send + 'static {
    const NAME: &'static str = "hannibal::Actor";

    /// Called when the actor is started.
    ///
    /// This method is async, the receiving of the first message is delayed until this method
    /// has completed.
    /// Returning an error will stop the actor.
    #[allow(unused)]
    fn started(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = DynResult> + Send {
        async { Ok(()) }
    }

    #[allow(unused)]
    fn stopped(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async {}
    }
}

/// Start building and configure you actor.
///
/// One feature of hannibal is that you can configure your actor's runtime beha is behaving before spawning.
/// This includes
/// ## 1. what kind of channels do you use under the hood /// and how large are the channel's buffers?
/// Unbounded versus Bounded.
///
/// ### Example
/// You can configure what kind of channel the actor uses and what its capacity should be.
///
/// ```no_run
/// # #[derive(hannibal_derive::Actor,hannibal_derive::RestartableActor, Default)]
/// # struct MyActor;
/// // start MyActor with a mailbox capacity of 6
/// let addr_bounded = hannibal::build(MyActor)
///     .bounded(6)
///     .spawn();
///
/// //  start MyActor with an infinite mailbox
/// let addr_unbounded = hannibal::build(MyActor)
///     .unbounded()
///     .spawn();
/// ```
/// ## 2. should the actor create a fresh object on restart?
/// If you restart the actor its [`started()`](`Actor::started`) method will be called.
/// You don't need to clean up and reset the actor's state if you configure it to be recreated from `Default` at spawn-time.
///
/// ### Example: Reset on Restart
/// This configuration will start the actor `Counter` with a bounded channel capacity of `6`
/// and recreate it from `Default` when `.restart()` is called.
/// ```no_run
/// # #[derive(hannibal_derive::Actor,hannibal_derive::RestartableActor)]
/// #[derive(Default)]
/// struct Counter(usize);
///
/// let addr = hannibal::build(Counter(0))
///     .bounded(6)
///     .recreate_from_default()
///     .spawn();
/// ```
///
/// ## 3. should it listen to a stream.
///
/// This one is an improvement over 0.10 where under the hood we would start listening to the stream on a separate task and send each message to the actor.
/// This ment two tasks and twice the amount of sending.
/// Since 0.12 hannibal can start the actor's event loop tightly coupled to the stream.
///
/// ### Example: Attache to stream
/// Here we tie the lifetime of the actor to the stream.
/// There is no extra task or messaging between them,
/// the actor owns the stream now and polls it in its own event loop
///
/// ```no_run
/// # use hannibal::{Context, StreamHandler, Handler, Actor};
/// struct Connector;
/// impl Actor for Connector {}
///
/// impl StreamHandler<i32> for Connector {
///     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
///         println!("[Connection] Received: {}", msg);
///     }
/// }
///
/// let the_stream = futures::stream::iter(0..19);
///
/// let addr = hannibal::build(Connector)
///         .unbounded()
///         .non_restartable()
///         .with_stream(the_stream)
///         .spawn();
///
/// # let the_stream = futures::stream::iter(0..19);
///
/// // you can also express this shorter
/// let addr = hannibal::build(Connector)
///         .on_stream(the_stream)
///         .spawn();
///
/// # let the_stream = futures::stream::iter(0..19);
///
/// // you can also have bounded channels
/// let addr = hannibal::build(Connector)
///         .bounded_on_stream(10, the_stream)
///         .spawn();
/// ```
/// ## 4. should the actor enfore timeouts when waiting?
///
///
///
/// ## Example: timeouts
/// ## Example: register service
/// ```no_run
/// # #[derive(hannibal_derive::Actor,hannibal_derive::RestartableActor)]
/// #[derive(Default)]
/// struct Counter(usize);
/// impl hannibal::Service for Counter {}
///
/// # async move {
/// let addr = hannibal::build(Counter(0))
///     .bounded(6)
///     .recreate_from_default()
///     .register()
///     .await
///     .unwrap();
/// # };
/// ```
/// Instead of spawning the actor, which will return you the actor's address you can also register it as a service.
#[cfg(any(feature = "tokio", feature = "async-std"))]
pub fn build<A: Actor>(actor: A) -> builder::BaseActorBuilder<A, spawner::DefaultSpawner> {
    builder::BaseActorBuilder::new(actor)
}

#[cfg(test)]
pub mod tests {
    pub struct Ping;
    pub struct Pong;
    impl crate::Message for Ping {
        type Response = Pong;
    }
    pub struct Identify;
    impl crate::Message for Identify {
        type Response = usize;
    }

    #[cfg(feature = "async-std")]
    pub mod spawned_with_asyncstd {
        use std::marker::PhantomData;

        use super::{Identify, Ping, Pong};
        use crate::{
            Handler, Service,
            actor::{Actor, Context},
        };

        #[derive(Debug, Default)]
        pub struct AsyncStdActor<T: Send + Sync + Default>(pub usize, pub PhantomData<T>);

        impl<T: Send + Sync + Default> AsyncStdActor<T> {
            pub fn new(value: usize) -> Self {
                Self(value, Default::default())
            }
        }

        impl<T: Send + Sync + Default + 'static> Actor for AsyncStdActor<T> {}
        impl<T: Send + Sync + Default + 'static> Service for AsyncStdActor<T> {}
        impl<T: Send + Sync + Default + 'static> Handler<Ping> for AsyncStdActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }
        impl<T: Send + Sync + Default + 'static> Handler<Identify> for AsyncStdActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Identify) -> usize {
                self.0
            }
        }
    }

    #[cfg(feature = "tokio")]
    pub mod spawned_with_tokio {
        use std::sync::{
            LazyLock,
            atomic::{AtomicUsize, Ordering},
        };

        use super::{Identify, Ping, Pong};
        use crate::{
            Handler, Service,
            actor::{Actor, Context},
        };

        #[derive(Debug)]
        pub struct TokioActor<T: Send + Sync + Default>(pub usize, pub std::marker::PhantomData<T>);

        impl<T: Send + Sync + Default> TokioActor<T> {
            pub fn new(value: usize) -> Self {
                Self(value, Default::default())
            }
        }

        impl<T: Send + Sync + Default> Default for TokioActor<T> {
            fn default() -> Self {
                static COUNTER: LazyLock<AtomicUsize> = LazyLock::new(Default::default);
                Self(COUNTER.fetch_add(1, Ordering::Relaxed), Default::default())
            }
        }

        impl<T: Send + Sync + Default + 'static> Actor for TokioActor<T> {}
        impl<T: Send + Sync + Default + 'static> Service for TokioActor<T> {}
        impl<T: Send + Sync + Default + 'static> Handler<Ping> for TokioActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }
        impl<T: Send + Sync + Default + 'static> Handler<Identify> for TokioActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Identify) -> usize {
                self.0
            }
        }
    }
}
