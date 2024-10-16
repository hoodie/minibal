#![allow(unused_imports)]
use std::{future::Future, pin::Pin, sync::Arc};

use crate::{Addr, StreamHandler};

use super::{Actor, DynResult};

pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

pub(crate) type DynJoiner<A> = Box<dyn Joiner<A>>;
pub trait Joiner<A: Actor>: Send + Sync {
    fn join(&mut self) -> JoinFuture<A>;
}

impl<A, F> Joiner<A> for F
where
    A: Actor,
    F: FnMut() -> JoinFuture<A>,
    F: Send + Sync,
{
    fn join(&mut self) -> JoinFuture<A> {
        self()
    }
}

pub trait Spawner<A: Actor> {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static;
}

pub trait SpawnableWith: Actor {
    fn spawn_with<S: Spawner<Self>>(self) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = crate::Environment::unbounded().launch(self);
        let joiner = S::spawn(event_loop);
        Ok((addr, joiner))
    }
}

impl<A: Actor> SpawnableWith for A {}

pub trait Spawnable<S: Spawner<Self>>: Actor {
    fn spawn(self) -> crate::error::Result<Addr<Self>> {
        Ok(self.spawn_and_get_joiner()?.0)
    }

    fn spawn_and_get_joiner(self) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = crate::Environment::unbounded().launch(self);
        let joiner = S::spawn(event_loop);
        Ok((addr, joiner))
    }
}

pub trait StreamSpawnable<S: Spawner<Self>, T>: Actor + StreamHandler<T::Item>
where
    T: futures::Stream + Unpin + Send + 'static,
    T::Item: 'static + Send,
    Self: StreamHandler<T::Item>,
{
    fn spawn_on_stream(self, stream: T) -> crate::error::Result<Addr<Self>> {
        Ok(self.spawn_on_stream_and_get_joiner(stream)?.0)
    }

    fn spawn_on_stream_and_get_joiner(
        self,
        stream: T,
    ) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = crate::Environment::unbounded().launch_on_stream(self, stream);
        let joiner = S::spawn(event_loop);
        println!("spawned");
        Ok((addr, joiner))
    }
}

pub trait DefaultSpawnable<S: Spawner<Self>>: Actor + Default {
    fn spawn_default() -> crate::error::Result<Addr<Self>> {
        Ok(Self::spawn_default_and_get_joiner()?.0)
    }

    fn spawn_default_and_get_joiner() -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = crate::Environment::unbounded().launch(Self::default());
        let joiner = S::spawn(event_loop);
        Ok((addr, joiner))
    }
}

#[cfg(feature = "tokio")]
#[derive(Copy, Clone, Debug, Default)]
pub struct TokioSpawner;
#[cfg(feature = "tokio")]
impl<A: Actor> Spawner<A> for TokioSpawner {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        let handle = Arc::new(async_lock::Mutex::new(Some(tokio::spawn(future))));
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                let mut handle: Option<tokio::task::JoinHandle<DynResult<A>>> =
                    handle.lock().await.take();

                if let Some(handle) = handle.take() {
                    // TODO: don't eat the error
                    handle.await.ok().and_then(Result::ok)
                } else {
                    None
                }
            })
        })
    }
}

// and now for async-std

#[cfg(feature = "async-std")]
#[derive(Copy, Clone, Debug, Default)]
pub struct AsyncStdSpawner;

#[cfg(feature = "async-std")]
impl<A: Actor> Spawner<A> for AsyncStdSpawner {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        let handle = Arc::new(async_lock::Mutex::new(Some(async_std::task::spawn(future))));
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                let mut handle: Option<async_std::task::JoinHandle<DynResult<A>>> =
                    handle.lock().await.take();

                if let Some(handle) = handle.take() {
                    // TODO: don 't eat the error
                    handle.await.ok()
                } else {
                    None
                }
            })
        })
    }
}

cfg_if::cfg_if! {
    if #[cfg( all(feature = "tokio", not(feature = "async-std")))] {
        impl<A> Spawnable<TokioSpawner> for A where A: Actor {}

        impl<A, T> StreamSpawnable<TokioSpawner, T> for A
        where
            A: Actor + StreamHandler<T::Item>,
            T: futures::Stream + Unpin + Send + 'static,
            T::Item: 'static + Send,
        {}

        #[cfg(feature = "tokio")]
        impl<A> DefaultSpawnable<TokioSpawner> for A where A: Actor + Default {}
    } else if #[cfg( all(not(feature = "tokio"), feature = "async-std") )] {
        impl<A> Spawnable<AsyncStdSpawner> for A where A: Actor {}

        impl<A, T> StreamSpawnable<AsyncStdSpawner, T> for A
        where
            A: Actor + StreamHandler<T::Item>,
            T: futures::Stream + Unpin + Send + 'static,
            T::Item: 'static + Send,
        {
        }

        impl<A> DefaultSpawnable<AsyncStdSpawner> for A where A: Actor + Default {}

    } else if #[cfg(all(feature = "tokio", feature = "async-std") )] {
        // if both are enabled, we can not provice a default spawner
    } else {
        // if both are enabled, we can not provice a default spawner either
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "tokio")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{spawned_with_tokio::TokioActor, Ping},
            spawn_strategy::{DefaultSpawnable, Spawnable, TokioSpawner},
        };

        #[tokio::test]
        async fn spawn() {
            let tokio_actor = TokioActor::default();
            let mut addr = <TokioActor as Spawnable<TokioSpawner>>::spawn(tokio_actor).unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[tokio::test]
        async fn spawn_default() {
            let mut addr = <TokioActor as DefaultSpawnable<TokioSpawner>>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }

    #[cfg(feature = "async-std")]
    mod spawned_with_asyncstd {
        use crate::{
            actor::tests::{spawned_with_asyncstd::AsyncStdActor, Ping},
            spawn_strategy::{AsyncStdSpawner, DefaultSpawnable, Spawnable},
        };

        #[async_std::test]
        async fn spawn() {
            let tokio_actor = AsyncStdActor::default();
            let mut addr =
                <AsyncStdActor as Spawnable<AsyncStdSpawner>>::spawn(tokio_actor).unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[async_std::test]
        async fn spawn_default() {
            let mut addr =
                <AsyncStdActor as DefaultSpawnable<AsyncStdSpawner>>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }
}
