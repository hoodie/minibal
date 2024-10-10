use std::future::Future;

use crate::{context::Context, Addr};

pub type ActorResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub trait Actor: Sized + Send + 'static {
    #[allow(unused)]
    fn started(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ActorResult> + Send {
        async { Ok(()) }
    }

    #[allow(unused)]
    fn stopped(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async {}
    }
}

mod service {
    #![allow(unused)]
    use std::{
        any::{Any, TypeId},
        collections::HashMap,
        pin::Pin,
        sync::{Arc, LazyLock},
    };

    use futures::FutureExt;
    use spawn_strategy::Spawner;

    use super::*;
    use crate::{error::ActorError::ServiceNotFound, Environment};

    type AnyBox = Box<dyn Any + Send + Sync>;

    pub trait Service<S>: Actor + Default
    where
        S: Spawner<Self>,
    {
        async fn from_registry() -> crate::error::Result<Addr<Self>> {
            static REGISTRY: LazyLock<async_lock::Mutex<HashMap<TypeId, AnyBox>>> =
                LazyLock::new(Default::default);

            let key = TypeId::of::<Self>();
            if let Some(addr) = REGISTRY
                .lock()
                .await
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
            {
                Ok(addr)
            } else {
                let (event_loop, addr) = Environment::unbounded().launch(Self::default());
                let mut joiner = S::spawn(event_loop.map(|fut| fut.unwrap()));
                Ok(addr)
            }
        }
    }
}

mod spawn_strategy {
    use std::{pin::Pin, sync::Arc};

    use super::*;
    pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

    pub(crate) trait Joiner<A: Actor>: Send + Sync {
        #[allow(unused)]
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

    pub(crate) trait Spawner<A: Actor> {
        fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
        where
            F: Future<Output = A> + Send + 'static;
    }

    #[derive(Debug, Default)]
    #[cfg(feature = "tokio")]
    pub struct TokioSpawner;
    impl<A: Actor> Spawner<A> for TokioSpawner {
        fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
        where
            F: Future<Output = A> + Send + 'static,
        {
            let handle = Arc::new(async_lock::Mutex::new(Some(tokio::spawn(future))));
            Box::new(move || -> JoinFuture<A> {
                let handle = Arc::clone(&handle);
                Box::pin(async move {
                    let mut handle: Option<tokio::task::JoinHandle<A>> = handle.lock().await.take();

                    if let Some(handle) = handle.take() {
                        handle.await.ok()
                    } else {
                        None
                    }
                })
            })
        }
    }

    #[cfg(feature = "async-std")]
    pub struct AsyncStdSpawner;
    impl<A: Actor> Spawner<A> for AsyncStdSpawner {
        fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
        where
            F: Future<Output = A> + Send + 'static,
        {
            let handle = Arc::new(async_lock::Mutex::new(Some(async_std::task::spawn(future))));
            Box::new(move || -> JoinFuture<A> {
                let handle = Arc::clone(&handle);
                Box::pin(async move {
                    let mut handle: Option<async_std::task::JoinHandle<A>> =
                        handle.lock().await.take();

                    if let Some(handle) = handle.take() {
                        Some(handle.await)
                    } else {
                        None
                    }
                })
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use service::Service;
    #[allow(unused)]
    use spawn_strategy::{AsyncStdSpawner, TokioSpawner};

    use crate::{Handler, Message};

    use super::*;

    #[tokio::test]
    async fn get_service_from_registry() {

        struct Ping;
        struct Pong;
        impl Message for Ping {
            type Result = Pong;
        }


        #[derive(Debug, Default)]
        struct AsyncStdServiceActor;
        impl Actor for AsyncStdServiceActor {}
        impl Service<AsyncStdSpawner> for AsyncStdServiceActor {}
        impl Handler<Ping> for AsyncStdServiceActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }

        #[derive(Debug, Default)]
        struct TokioServiceActor;
        impl Actor for TokioServiceActor {}
        impl Service<TokioSpawner> for TokioServiceActor {}
        impl Handler<Ping> for TokioServiceActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }

        let mut addr_asyncstd = AsyncStdServiceActor::from_registry().await.unwrap();
        assert!(!addr_asyncstd.stopped());

        let mut addr_tokio = TokioServiceActor::from_registry().await.unwrap();
        assert!(!addr_tokio.stopped());


        addr_asyncstd.call(Ping).await.unwrap();
        addr_asyncstd.stop().unwrap();
        addr_asyncstd.await.unwrap();

        addr_tokio.call(Ping).await.unwrap();
        addr_tokio.stop().unwrap();
        addr_tokio.await.unwrap()
    }
}
