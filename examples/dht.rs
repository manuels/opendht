#![feature(await_macro, async_await)]

extern crate futures;
extern crate opendht;
extern crate tokio;

use futures::compat::*;
use futures::prelude::*;
use std::net::ToSocketAddrs;

use opendht::OpenDht;

async fn run(dht: OpenDht) {
    println!("Bootstrapping...");
    let addrs: Vec<_> = "bootstrap.ring.cx:4222"
        .to_socket_addrs()
        .unwrap()
        .collect();
    let f = dht.bootstrap(&addrs);
    await!(f).unwrap();

    let key = &b"foo"[..];

    println!("Storing...");
    let f = dht.put(key, &[9, 9, 9]);
    await!(f).unwrap();

    let mut f = dht.get(key);

    while let Some(item) = await!(f.next()) {
        println!("Found {:?}", item);
    }

    dht.join();
    println!("Done: All threads joined.");
}

fn main() {
    let f = async {
        let dht = OpenDht::new(4222).unwrap();

        let dht2 = dht.clone();

        let f = async move {
            while let Some(next) = dht2.tick() {
                let f = tokio::timer::Delay::new(next);
                let _ = await!(f.compat());
            }
        };
        tokio::spawn(f.boxed().unit_error().compat());

        await!(run(dht));
    };

    tokio::run(f.boxed().unit_error().compat());
}
