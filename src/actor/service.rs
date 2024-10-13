use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::LazyLock,
};

use super::spawn_strategy::Spawner;
use super::*;

use crate::{Addr, Environment};

type AnyBox = Box<dyn Any + Send + Sync>;

static REGISTRY: LazyLock<async_lock::Mutex<HashMap<TypeId, AnyBox>>> =
    LazyLock::new(Default::default);

impl<A: Actor> Addr<A>
where
    A: Service,
{
    pub fn register(self) -> impl Future<Output = Option<Addr<A>>> {
        async {
            let key = TypeId::of::<Self>();
            REGISTRY
                .lock()
                .await
                .insert(key, Box::new(self))
                .and_then(|addr| addr.downcast::<Addr<A>>().ok())
                .map(|addr| *addr)
        }
    }
}

pub trait Service: Actor + Default {
    fn from_registry() -> impl Future<Output = crate::error::Result<Addr<Self>>> {
        <Self as SpawnableService<spawn_strategy::TokioSpawner>>::from_registry()
    }
}

#[cfg(feature = "tokio")]
impl<A> SpawnableService<spawn_strategy::TokioSpawner> for A where A: Service {}

pub trait SpawnableService<S>: Actor + Default
where
    S: Spawner<Self>,
{
    fn from_registry() -> impl Future<Output = crate::error::Result<Addr<Self>>> {
        async {
            let key = TypeId::of::<Self>();

            let mut entry = REGISTRY.lock().await;

            if let Some(addr) = entry
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
            {
                Ok(addr)
            } else {
                let (event_loop, addr) = Environment::unbounded().launch(Self::default());
                S::spawn(event_loop);
                entry.insert(key, Box::new(addr.clone()));
                Ok(addr)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "tokio")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{spawned_with_tokio::TokioActor, Identify, Ping},
            service::SpawnableService,
            spawn_strategy::{SpawnableWith, TokioSpawner},
        };

        #[tokio::test]
        async fn register_as_service() {
            let (addr, mut joiner) = TokioActor(1337).spawn_with::<TokioSpawner>().unwrap();
            addr.register().await;
            let mut svc_addr = TokioActor::from_registry().await.unwrap();
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);

            svc_addr.stop().unwrap();
            joiner.join().await.unwrap();
        }

        #[tokio::test]
        async fn get_service_from_registry() {
            let mut svc_addr = TokioActor::from_registry().await.unwrap();
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();

            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }
    }

    #[cfg(feature = "async-std")]
    mod spawned_with_asyncstd {
        use crate::{
            actor::tests::{spawned_with_asyncstd::AsyncStdActor, Identify, Ping},
            service::SpawnableService,
            spawn_strategy::{AsyncStdSpawner, SpawnableWith},
        };

        #[tokio::test]
        async fn register_as_service() {
            let (addr, mut joiner) = AsyncStdActor(1337).spawn_with::<AsyncStdSpawner>().unwrap();
            addr.register().await;
            let mut svc_addr = AsyncStdActor::from_registry().await.unwrap();
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);

            svc_addr.stop().unwrap();
            joiner.join().await.unwrap();
        }

        #[async_std::test]
        async fn get_service_from_registry() {
            let mut svc_addr = AsyncStdActor::from_registry().await.unwrap();
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();
            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }
    }
}
