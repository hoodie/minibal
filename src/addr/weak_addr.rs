use std::sync::Arc;

use dyn_clone::DynClone;

use crate::context::RunningFuture;
#[allow(unused)]
use crate::{
    channel::{ChanTx, WeakChanTx},
    error::ActorError::AlreadyStopped,
    Actor, Addr, Handler,
};

pub struct WeakAddr<A: Actor> {
    pub(super) upgrade: Box<dyn UpgradeFn<A>>,
    pub(crate) running: RunningFuture,
}

impl<A: Actor> WeakAddr<A> {
    pub fn upgrade(&self) -> Option<Addr<A>> {
        self.upgrade.upgrade()
    }

    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }
}

impl<A: Actor> From<&Addr<A>> for WeakAddr<A> {
    fn from(addr: &Addr<A>) -> Self {
        let weak_tx = Arc::downgrade(&addr.payload_tx);
        let running = addr.running.clone();
        let running_inner = addr.running.clone();
        let upgrade = Box::new(move || {
            let running = running_inner.clone();
            weak_tx.upgrade().map(|payload_tx| Addr {
                payload_tx,
                running,
            })
        });

        WeakAddr { upgrade, running }
    }
}

impl<A: Actor> Clone for WeakAddr<A> {
    fn clone(&self) -> Self {
        WeakAddr {
            upgrade: dyn_clone::clone_box(&*self.upgrade),
            running: self.running.clone(),
        }
    }
}

pub(super) trait UpgradeFn<A: Actor>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Addr<A>>;
}

impl<F, A> UpgradeFn<A> for F
where
    F: Fn() -> Option<Addr<A>>,
    F: 'static + Send + Sync + Clone,
    A: Actor,
{
    fn upgrade(&self) -> Option<Addr<A>> {
        self()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::tests::*;

    #[tokio::test]
    async fn upgrade() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        assert_eq!(weak_addr.upgrade().unwrap().call(Add(1, 2)).await, Ok(3))
    }

    #[tokio::test]
    async fn does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        weak_addr.upgrade().unwrap();

        drop(addr);

        actor.await.unwrap().unwrap();
        assert!(weak_addr.upgrade().is_none());
    }

    #[tokio::test]
    async fn send_fails_after_drop() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        let mut addr = weak_addr.upgrade().unwrap();
        addr.stop().unwrap();

        actor.await.unwrap().unwrap();
        assert!(addr.send(Store("password")).is_err());
    }
}
