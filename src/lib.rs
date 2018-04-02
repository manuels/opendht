#![feature(proc_macro, pin, generators)]

extern crate libc;
extern crate nix;
extern crate futures_await as futures;
extern crate tokio;
extern crate tokio_timer;

mod sys;

use std::net::SocketAddr;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use std::sync::Arc;
use std::sync::Mutex;

use nix::sys::socket::InetAddr;
use nix::sys::socket::SockAddr;
use futures::prelude::*;
use futures::sync::oneshot;
use futures::sync::mpsc;

#[derive(Clone)]
pub struct OpenDht(Arc<Mutex<sys::dht_t>>);
unsafe impl Send for OpenDht {}
unsafe impl Sync for OpenDht {}

extern fn done_callback(success: sys::c_bool, tx: *mut oneshot::Sender<bool>)
{
    let tx = unsafe { Box::from_raw(tx) };
    let _ = tx.send(success != 0);
}

extern fn get_callback(values: *mut *mut libc::c_uchar,
    lengths: *const libc::size_t,
    count: libc::size_t,
    tx: *mut mpsc::Sender<Vec<u8>>)
    -> sys::c_bool
{
    let mut tx = unsafe { Box::from_raw(tx) };
    let ptr_list = unsafe { std::slice::from_raw_parts(values, count) };
    let len_list = unsafe { std::slice::from_raw_parts(lengths as *mut libc::size_t, count) };

    for (ptr, len) in ptr_list.iter().zip(len_list.iter()) {
        let item = unsafe { std::slice::from_raw_parts(*ptr, *len)};

        if tx.try_send(item.to_vec()).is_err() {
            std::mem::forget(tx);
            std::mem::forget(item);
            std::mem::forget(ptr_list);
            std::mem::forget(len_list);
            return 0;
        }

        std::mem::forget(item);
    }

    std::mem::forget(tx);
    std::mem::forget(ptr_list);
    std::mem::forget(len_list);
    return 1;
}

fn convert_socketaddr(s: &SocketAddr) -> libc::sockaddr {
    let s = InetAddr::from_std(s);
    let s = SockAddr::new_inet(s);
    unsafe {
        std::mem::transmute(*s.as_ffi_pair().0)
    }
}

impl OpenDht {
    /// Starts a new DHT client.
    ///
    /// # Arguments
    ///
    /// * `port` - UDP port to use for network communication
    pub fn new(port: u16) -> OpenDht {
        let ptr = unsafe { sys::dht_init() };
        unsafe { sys::dht_run(ptr, port) };

        OpenDht(Arc::new(Mutex::new(ptr)))
    }

    /// Connect this DHT client to other nodes.
    ///
    /// # Remarks
    ///
    /// Usually you should use `bootstrap.ring.cx` on port `4222` here.
    /// You MUST call bootstrap to make your DHT client to work (unless you are
    /// running a bootstrapping node - for experts only).
    pub fn bootstrap(&self, sockets: &[SocketAddr])
        -> oneshot::Receiver<bool>
    {
        let (tx, rx) = oneshot::channel();
        let tx = Box::new(tx);
        let tx = Box::into_raw(tx);

        let socks: Vec<libc::sockaddr>;
        socks = sockets.iter().map(convert_socketaddr).collect();

        let ptr = socks.as_ptr();
        let this = self.0.lock().unwrap();

        unsafe {
            sys::dht_bootstrap(*this, ptr, socks.len(), done_callback, tx);
        }

        rx
    }

    fn tick(ptr: sys::dht_t) -> Duration {
        let next = unsafe { sys::dht_loop(ptr) };
        let next = UNIX_EPOCH + Duration::from_secs(next as _);
        next.duration_since(SystemTime::now()).unwrap_or(Duration::from_millis(200))
    }

    /// Wait for DHT threads to end. Run this before your program ends.
    pub fn join(&self) {
        let this = self.0.lock().unwrap();
        unsafe { sys::dht_join(*this) };
    }

    /// Put a value on the DHT.
    ///
    /// # Arguments
    ///
    /// * `key` - Key that is used to find the value by the other DHT clients.
    /// * `value` - Value to store on the DHT.
    pub fn put(&self, key: &[u8], value: &[u8])
        -> oneshot::Receiver<bool>
    {
        let (tx, rx) = oneshot::channel();
        let tx = Box::new(tx);
        let tx = Box::into_raw(tx);

        let key_ptr = key.as_ptr();
        let ptr = value.as_ptr();

        let this = self.0.lock().unwrap();

        unsafe {
            sys::dht_put(*this, key_ptr, key.len(), ptr, value.len(),
                done_callback, tx);
        }

        rx
    }

    /// Get a value from the DHT. This function returns a
    /// [Stream](futures::stream::Stream) of values that are found.
    /// The stream MAY contain duplicates.
    ///
    /// # Arguments
    ///
    /// * `key` - Key to lookup.
    pub fn get(&self, key: &[u8]) -> mpsc::Receiver<Vec<u8>>
    {
        extern fn get_done_callback(_success: sys::c_bool, tx: *mut mpsc::Sender<Vec<u8>>)
        {
            let tx = unsafe { Box::from_raw(tx) };
            drop(tx);
        }

        let (get_tx, get_rx) = mpsc::channel(10);
        let get_tx = Box::new(get_tx);
        let get_tx = Box::into_raw(get_tx);

        let key_ptr = key.as_ptr();
        let this = self.0.lock().unwrap();

        unsafe {
            sys::dht_get(*this, key_ptr, key.len(), get_callback, get_tx,
                get_done_callback, get_tx);
        }

        get_rx
    }

    /// Listen for values published on the DHT. This function returns a
    /// [Stream](futures::stream::Stream) of values that are currently on the
    /// DHT and also returns new values as soon as they are published.
    /// The stream never ends and MAY contain duplicates.
    ///
    /// # Arguments
    ///
    /// * `key` - Key to lookup.
    pub fn listen(&self, key: &[u8]) -> mpsc::Receiver<Vec<u8>>
    {
        let (get_tx, get_rx) = mpsc::channel(10);
        let get_tx = Box::new(get_tx);
        let get_tx = Box::into_raw(get_tx);

        let key_ptr = key.as_ptr();
        let this = self.0.lock().unwrap();

        unsafe {
            sys::dht_listen(*this, key_ptr, key.len(), get_callback, get_tx);
        }

        get_rx
    }

    /// Runs maintainance tasks. Returns a [Future](futures::future::Future)
    /// that never returns, so you probably should `clone` this `OpenDht` instance
    /// and [tokio::spawn](tokio::executor::spawn) it.
    #[async]
    pub fn maintain(dht: Self) -> Result<(),()> {
        // TODO: quit on drop
        loop {
            let next = {
                let ptr = dht.0.lock().unwrap();
                let next = OpenDht::tick(*ptr);
                drop(ptr);
                next
            };

            let f = tokio_timer::Timer::default().sleep(next);
            await!(f.map_err(|_| ()))?;
        }
    }

    /// Returns a serialized list of dht clients that this client knows.
    /// You can store this to a file when your program ends and `deserialize()`
    /// when your program starts again.
    pub fn serialize(&self) -> Vec<u8> {
        extern fn cb(src: *const libc::c_uchar, len: libc::size_t, dst: *mut Vec<u8>) {
            unsafe {
                let src = std::slice::from_raw_parts(src, len);
                (*dst).copy_from_slice(&src);
            }
        };

        let buf = Box::new(Vec::new());
        let ptr = Box::into_raw(buf);
        let this = self.0.lock().unwrap();

        let vec = unsafe {
            sys::serialize(*this, cb, ptr);
            Box::from_raw(ptr)
        };

        *vec
    }

    /// Deserializes a list of dht clients and adds them to the routing table.
    /// (See `serialize()`)
    pub fn deserialize(&self, buf: &[u8]) {
        let ptr = buf.as_ptr();
        let this = self.0.lock().unwrap();

        unsafe {
            sys::deserialize(*this, ptr, buf.len());
        }
    } 
}

impl Drop for OpenDht {
    fn drop(&mut self) {
        unsafe {
            let this = self.0.lock().unwrap();
            sys::dht_drop(*this)
        }
    }
}
