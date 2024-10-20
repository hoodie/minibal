use std::future::Future;

use crate::{context::Context, Actor, DynResult};

pub trait RestartStrategy<A> {
    fn refresh(actor: A, ctx: &mut Context<A>) -> impl Future<Output = DynResult<A>> + Send;
}

#[derive(Clone, Copy, Debug)]
pub struct RestartOnly;

#[derive(Clone, Copy, Debug)]
pub struct RecreateFromDefault;

impl<A> RestartStrategy<A> for RestartOnly
where
    A: Actor,
{
    async fn refresh(mut actor: A, ctx: &mut Context<A>) -> DynResult<A> {
        eprintln!("restarting refresh");
        actor.stopped(ctx).await;
        actor.started(ctx).await?;
        Ok(actor)
    }
}

impl<A> RestartStrategy<A> for RecreateFromDefault
where
    A: Actor + Default,
{
    async fn refresh(mut actor: A, ctx: &mut Context<A>) -> DynResult<A> {
        eprintln!("recreating refresh");
        actor.stopped(ctx).await;
        actor = A::default();
        actor.started(ctx).await?;
        Ok(actor)
    }
}

pub trait Restartable: Actor {}
