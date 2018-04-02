#![feature(proc_macro, pin, generators)]

extern crate opendht;
extern crate tokio;
extern crate tokio_timer;
extern crate futures_await as futures;

use std::net::ToSocketAddrs;
use futures::prelude::*;

use opendht::OpenDht;

#[async]
fn run() -> Result<(),()> {
    let dht = OpenDht::new(4222);

    let f = OpenDht::maintain(dht.clone());
    tokio::spawn(f);
    
    println!("Bootstrapping...");
    let addrs: Vec<_> = "bootstrap.ring.cx:4222".to_socket_addrs().unwrap().collect();
    let f = dht.bootstrap(&addrs);
    await!(f).unwrap();

    println!("Storing...");
    let f = dht.put(&[6;20], &[9,9,9]);
    await!(f).unwrap();

    let f = dht.get(&[6;20]);
    #[async]
    for item in f {
        println!("Found {:?}", item);
    }

    dht.join();
    println!("Done: All threads joined.");

    Ok(())
}

fn main() {
    tokio::run(run());
}
