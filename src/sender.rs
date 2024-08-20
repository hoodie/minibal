use std::{
    marker::PhantomData,
    sync::{mpsc, Arc, RwLock, Weak},
};

use crate::{error::ActorError::WriteError, event_loop::Payload, Addr, Handler};

#[derive(Clone)]
pub struct Sender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: Arc<RwLock<dyn Handler<M>>>,
    marker: PhantomData<M>,
}

impl<M, A> From<Addr<A>> for Sender<M>
where
    A: Handler<M> + 'static,
{
    fn from(Addr { ctx, actor }: Addr<A>) -> Self {
        Sender {
            tx: ctx.tx.clone(),
            actor: actor.clone(),
            marker: PhantomData,
        }
    }
}

impl<M> Sender<M> {
    pub fn send(&self, msg: M)
    where
        M: Send + Sync + 'static,
    {
        let actor = self.actor.clone();
        self.tx
            .send(Payload::from(move || {
                actor.write().map_err(|_| WriteError)?.handle(msg);
                Ok(())
            }))
            .unwrap()
    }

    pub fn downgrade(&self) -> WeakSender<M> {
        WeakSender {
            tx: Arc::downgrade(&self.tx),
            actor: Arc::downgrade(&self.actor),
            marker: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct WeakSender<M> {
    tx: Weak<mpsc::Sender<Payload>>,
    actor: Weak<RwLock<dyn Handler<M>>>,
    marker: PhantomData<M>,
}

impl<M> WeakSender<M> {
    pub fn try_send(&self, msg: M) -> bool
    where
        M: Send + Sync + 'static,
    {
        if let Some((tx, actor)) = self.tx.upgrade().zip(self.actor.upgrade()) {
            tx.send(Payload::from(move || {
                actor.write().map_err(|_| WriteError)?.handle(msg);
                Ok(())
            }))
            .unwrap();
            true
        } else {
            eprintln!("Actor is dead");
            false
        }
    }
}
