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
        sync::LazyLock,
    };

    use super::*;
    use crate::error::ActorError::ServiceNotFound;

    type AnyBox = Box<dyn Any + Send + Sync>;

    pub trait Service: Actor + Default {
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
                todo!();
            }
        }
    }

    pub type JoinFuture<A> = Pin<Box<dyn Future<Output = A> + Send>>;

    pub(crate) trait Joiner<A: Actor> {
        fn join(self) -> JoinFuture<A>;
    }

    impl<A, F> Joiner<A> for F
    where
        A: Actor,
        F: FnOnce() -> JoinFuture<A>,
        F: Send + Sync,
    {
        fn join(self) -> JoinFuture<A> {
            self()
        }
    }

    pub(crate) trait Spawner<A: Actor> {
        fn spawn<F>(&self, future: F) -> Box<dyn Joiner<A>>
        where
            F: Future<Output = A> + Send + 'static;
    }

    struct TokioSpawner;
    impl<A: Actor> Spawner<A> for TokioSpawner {
        fn spawn<F>(&self, future: F) -> Box<dyn Joiner<A>>
        where
            F: Future<Output = A> + Send + 'static,
        {
            let handle = tokio::spawn(future);
            Box::new(move || -> JoinFuture<A> {
                Box::pin(async move {
                    let actor = handle.await.unwrap();
                    actor
                })
            })
        }
    }
}
