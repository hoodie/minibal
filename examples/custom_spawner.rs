use std::{future::Future, sync::Arc};

use minibal::prelude::*;
use minibal::spawn_strategy::{JoinFuture, Joiner, Spawner};

#[derive(Copy, Clone, Debug, Default)]
struct CustomSpawner;

impl<A: Actor> Spawner<A> for CustomSpawner {
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

struct MyActor(&'static str);
impl Actor for MyActor {}

impl Spawnable<CustomSpawner> for MyActor {}

#[cfg(all(not(feature = "tokio"), not(feature = "async-std")))]
fn main() {
    MyActor("Hello, world!").spawn();
    todo!();
}
