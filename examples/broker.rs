use futures::future::join;
use hannibal::{Broker, prelude::*};

#[derive(Clone, Message)]
struct Topic1(u32);

#[derive(Debug, Default, PartialEq)]
struct Subscribing1(Vec<u32>);

#[derive(Debug, Default, PartialEq)]
struct Subscribing2(Vec<u32>);

impl Actor for Subscribing1 {
    const NAME: &'static str = "Subscribing1";
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.subscribe::<Topic1>().await?;
        Ok(())
    }
}

impl Actor for Subscribing2 {
    const NAME: &'static str = "Subscribing2";
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        ctx.subscribe::<Topic1>().await?;
        Ok(())
    }
}

impl Handler<Topic1> for Subscribing1 {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: Topic1) {
        log::debug!("{} adding {:?}", Self::NAME, msg.0);
        self.0.push(msg.0);
        if self.0.len() == 2 {
            log::info!("{} stopping", Self::NAME);
            ctx.stop().unwrap();
        }
    }
}
impl Handler<Topic1> for Subscribing2 {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: Topic1) {
        log::debug!("{} adding {:?}", Self::NAME, msg.0);
        self.0.push(msg.0);
        if self.0.len() == 2 {
            log::info!("{} stopping", Self::NAME);
            ctx.stop().unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let mut subscriber1 = Subscribing1::default().spawn_owning();
    let mut subscriber2 = Subscribing2::default().spawn_owning();

    // avoid race condition in example
    let ping_both = || join(subscriber1.ping(), subscriber2.ping());

    let _ = ping_both().await;
    log::info!("both pinged");

    let broker = Broker::from_registry().await;
    broker.publish(Topic1(42)).await.unwrap();
    broker.publish(Topic1(23)).await.unwrap();

    // avoid race condition in example
    let _ = ping_both().await;
    log::info!("both pinged");

    assert_eq!(
        join(subscriber1.join(), subscriber2.join()).await,
        (
            Some(Subscribing1(vec![42, 23])),
            Some(Subscribing2(vec![42, 23]))
        )
    );
    // assert_eq!(subscriber1.consume().await, Ok(Subscribing1(vec![42, 23])));
    // assert_eq!(subscriber2.consume().await, Ok(Subscribing2(vec![42, 23])));
    println!("both subscribers received all messges");
}
