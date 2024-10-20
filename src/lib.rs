#![cfg_attr(test, allow(clippy::unwrap_used))]

mod actor;
mod addr;
mod channel;
mod context;
mod environment;
pub mod error;

mod handler;

pub use self::{
    actor::{
        service::{self, Service},
        spawn_strategy, Actor, DynResult,
    },
    addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
    context::Context,
    environment::{Environment, Restartable},
    handler::{Handler, StreamHandler},
};

pub mod prelude {
    pub use crate::{
        actor::{service::Service, Actor, DynResult},
        addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
        context::Context,
        handler::{Handler, StreamHandler},
        spawn_strategy::{Spawnable, StreamSpawnable},
    };
}
