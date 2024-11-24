#![cfg(feature = "tokio")]
use minibal::prelude::*;

#[derive(Debug)]
struct MyActor(&'static str);

struct Greet(&'static str);
impl Message for Greet {
    type Result = ();
}

struct Add(i32, i32);
impl Message for Add {
    type Result = i32;
}

impl Actor for MyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("[Actor {}] started", self.0);
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("[Actor {}] stopped", self.0);
    }
}

impl Handler<Greet> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Greet) {
        println!(
            "[Actor {me}] Hello {you}, my name is {me}",
            me = self.0,
            you = msg.0,
        );
    }
}

impl Handler<Add> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
        msg.0 + msg.1
    }
}

#[tokio::main]
async fn main() {
    let addr = MyActor("Caesar").spawn_owning();
    addr.as_addr().send(Greet("Cornelius")).unwrap();
    let addition = addr.as_addr().call(Add(1, 2)).await;

    println!("The Actor Calculated: {:?}", addition);
    println!("{:#?}", addr.stop_and_join().await);
}