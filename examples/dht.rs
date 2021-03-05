extern crate futures;
extern crate opendht;

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
    dht.bootstrap(&addrs).await.unwrap();

    let key = &b"foo"[..];

    println!("Storing...");
    dht.put(key, &[9, 9, 9]).await.unwrap();

    let mut f = dht.get(key);
//    let mut f = dht.listen(key);

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
                use async_std::prelude::*;
                futures::future::ready(()).delay(next).await;
            }
        };
        async_std::task::spawn(f);

        run(dht).await;
    };

    futures::executor::block_on(f);
}
