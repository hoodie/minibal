use std::{future::Future, sync::Arc};

use futures_executor::LocalPool;
use minibal::prelude::*;
use minibal::spawn_strategy::{JoinFuture, Joiner, Spawner};

#[derive(Debug, Default)]
struct CustomSpawner(LocalPool);

impl<A: Actor> Spawner<A> for CustomSpawner {
    const NAME: &'static str = "CustomSpawner";
    fn spawn<F>(&self, future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        let handle = Arc::new(async_lock::Mutex::new(Some(self.0.spawn_ok(future))));
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

struct MyActor;
impl Actor for MyActor {}

impl Spawnable<CustomSpawner> for MyActor {}

#[cfg(all(not(feature = "tokio"), not(feature = "async-std")))]
fn main() {
    futures::executor::block_on(async {
        let mut addr = MyActor.spawn().unwrap();
        addr.stop().unwrap();
        addr.await.unwrap();
    })
}
