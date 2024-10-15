use std::{borrow::Borrow as _, future::Future, sync::Arc};

use futures::{executor::LocalPool, task::SpawnExt};
use minibal::{
    prelude::*,
    spawn_strategy::{JoinFuture, Joiner, SpawnableWith, Spawner},
};

thread_local! {
    static POOL: std::cell::RefCell<LocalPool>  = LocalPool::new().into();
    // static LOCAL_REGISTRY: RefCell<HashMap<TypeId, Box<dyn Any + Send>, BuildHasherDefault<FnvHasher>>> = RefCell::default();
}

struct CustomSpawner;

impl<A: Actor> Spawner<A> for CustomSpawner {
    const NAME: &'static str = "CustomSpawner";

    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        eprintln!("Spawning actor with custom spawner");
        let handle = Arc::new(async_lock::Mutex::new(Some(
            POOL.borrow()
                .with(|pool| pool.borrow().spawner().spawn_with_handle(future)),
        )));
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                let mut handle = handle.lock().await.take().and_then(Result::ok);

                if let Some(handle) = handle.take() {
                    // TODO: don't eat the error
                    handle.await.ok()
                } else {
                    None
                }
            })
        })
    }
}

struct MyActor;
impl Actor for MyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult {
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        eprintln!("Actor stopped")
    }
}

// impl Spawnable<CustomSpawner> for MyActor {}

// #[cfg(all(not(feature = "tokio"), not(feature = "async-std")))]
fn main() {
    futures::executor::block_on(async {
        // let mut addr = MyActor.spawn().unwrap();
        let (mut addr, _) = MyActor.spawn_with::<CustomSpawner>().unwrap();
        // eprintln!("Actor started with {}", MyActor::spawner_name());

        addr.stop().unwrap();
        eprintln!("Actor asked to stop");
        addr.await.unwrap();
    })
}
