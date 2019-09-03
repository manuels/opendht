extern crate futures;
extern crate opendht;
extern crate tokio;

use futures::compat::*;
use futures::prelude::*;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use opendht::OpenDht;

async fn run(dht: Arc<OpenDht>) {
    println!("Bootstrapping...");
    let addrs: Vec<_> = "bootstrap.ring.cx:4222"
        .to_socket_addrs()
        .unwrap()
        .collect();
    let f = dht.bootstrap(&addrs);
    f.await.unwrap();

    let key = &b"foo"[..];

    println!("Storing...");
    let f = dht.put(key, &[9, 9, 9]);
    f.await.unwrap();

    let mut f = dht.get(key);

    while let Some(item) = f.next().await {
        println!("Found {:?}", item);
    }

    dht.join();
    println!("Done: All threads joined.");
}

fn main() {
    let f = async {
        let dht = Arc::new(OpenDht::new(4222).unwrap());
        let dht2 = dht.clone();

        let f = async move {
            while let Some(next) = dht2.tick() {
                let f = tokio::timer::Delay::new(next);
                let _ = f.compat().await;
            }
        };
        tokio::spawn(f.boxed().unit_error().compat());

        run(dht).await;
    };

    tokio::run(f.boxed().unit_error().compat());
}
