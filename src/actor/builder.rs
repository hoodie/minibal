use std::marker::PhantomData;

use crate::{actor::service::Service, channel::Channel, environment, Addr, StreamHandler};

use super::{
    restart_strategy::{NonRestartable, RecreateFromDefault, RestartOnly, RestartStrategy},
    spawn_strategy::Spawner,
    Actor, RestartableActor,
};

#[derive(Default)]
pub struct BaseActorBuilder<A, P>
where
    A: Actor,
    P: Spawner<A>,
{
    actor: A,
    spawner: PhantomData<P>,
}

pub struct ActorBuilderWithChannel<A: Actor, P, R: RestartStrategy<A>>
where
    A: Actor,
    P: Spawner<A>,
{
    base: BaseActorBuilder<A, P>,
    channel: Channel<A>,
    restart: PhantomData<R>,
}

pub struct StreamActorBuilder<A, P, S>
where
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
    A: StreamHandler<S::Item>,
    P: Spawner<A>,
{
    with_channel: ActorBuilderWithChannel<A, P, NonRestartable>,
    stream: S,
}

/// add channel
impl<A, P> BaseActorBuilder<A, P>
where
    A: Actor,
    P: Spawner<A>,
{
    pub(crate) const fn new(actor: A) -> Self {
        Self {
            actor,
            spawner: PhantomData,
        }
    }

    const fn with_channel(self, channel: Channel<A>) -> ActorBuilderWithChannel<A, P, RestartOnly> {
        ActorBuilderWithChannel {
            base: self,
            restart: PhantomData,
            channel,
        }
    }

    pub fn bounded(self, capacity: usize) -> ActorBuilderWithChannel<A, P, RestartOnly> {
        self.with_channel(Channel::bounded(capacity))
    }

    pub fn unbounded(self) -> ActorBuilderWithChannel<A, P, RestartOnly> {
        self.with_channel(Channel::unbounded())
    }
}

/// add stream
impl<A, P> ActorBuilderWithChannel<A, P, NonRestartable>
where
    A: Actor,
    P: Spawner<A>,
{
    pub fn with_stream<S>(self, stream: S) -> StreamActorBuilder<A, P, S>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        StreamActorBuilder {
            with_channel: self.non_restartable(),
            stream,
        }
    }
}

/// make non restartable
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: Actor,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub fn non_restartable(self) -> ActorBuilderWithChannel<A, P, NonRestartable> {
        ActorBuilderWithChannel {
            base: self.base,
            channel: self.channel,
            restart: PhantomData,
        }
    }
}

/// make recreate from `Default` on restart
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: RestartableActor + Default,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub fn recreate_from_default(self) -> ActorBuilderWithChannel<A, P, RecreateFromDefault> {
        ActorBuilderWithChannel {
            base: self.base,
            channel: self.channel,
            restart: PhantomData,
        }
    }
}

/// spawn actor
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: Actor,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub fn spawn(self) -> Addr<A> {
        let env = environment::Environment::<A, R>::from_channel(self.channel);
        let (event_loop, addr) = env.launch(self.base.actor);
        let _joiner = P::spawn_actor(event_loop);
        addr
    }
}

/// register service
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: Actor + Service,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub async fn register(self) -> crate::error::Result<(Addr<A>, Option<Addr<A>>)> {
        self.spawn().register().await
    }
}

/// spawn actor on stream, non restartable
impl<A, P, S> StreamActorBuilder<A, P, S>
where
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
    A: StreamHandler<S::Item>,
    A: Actor,
    P: Spawner<A>,
{
    pub fn spawn(self) -> Addr<A> {
        let Self {
            with_channel:
                ActorBuilderWithChannel {
                    base: BaseActorBuilder { actor, .. },
                    channel,
                    ..
                },
            stream,
        } = self;

        let env = environment::Environment::<A, NonRestartable>::from_channel(channel);
        let (event_loop, addr) = env.launch_on_stream(actor, stream);
        let _joiner = P::spawn_actor(event_loop);
        addr
    }
}
