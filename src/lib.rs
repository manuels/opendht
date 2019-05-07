#![feature(await_macro, async_await)]

extern crate futures;
extern crate libc;
extern crate nix;
extern crate ring;

mod sys;

use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;

use futures::channel::mpsc;
use futures::channel::oneshot;
use nix::sys::socket::InetAddr;
use nix::sys::socket::SockAddr;

use ring::digest;

pub struct InfoHash(digest::Digest);

impl InfoHash {
    pub fn new<T: AsRef<[u8]>>(key: T) -> InfoHash {
        let hash = digest::digest(&digest::SHA1, key.as_ref());
        InfoHash(hash)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ref().as_ptr()
    }

    pub fn len(&self) -> usize {
        self.0.as_ref().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_empty()
    }
}

impl<'a, T: Into<&'a [u8]>> From<T> for InfoHash {
    fn from(data: T) -> InfoHash {
        InfoHash::new(data.into())
    }
}

pub struct OpenDht(*mut libc::c_void);

unsafe impl Send for OpenDht {}
unsafe impl Sync for OpenDht {}

extern "C" fn done_callback(success: sys::c_bool, tx: *mut oneshot::Sender<bool>) {
    let tx = unsafe { Box::from_raw(tx) };
    let _ = tx.send(success != 0);
}

extern "C" fn get_callback(
    values: *mut *mut libc::c_uchar,
    lengths: *const libc::size_t,
    count: libc::size_t,
    tx: *mut mpsc::Sender<Vec<u8>>,
) -> sys::c_bool {
    let mut tx = unsafe { Box::from_raw(tx) };
    let ptr_list = unsafe { std::slice::from_raw_parts(values, count) };
    let len_list = unsafe { std::slice::from_raw_parts(lengths as *mut libc::size_t, count) };

    for (ptr, len) in ptr_list.iter().zip(len_list.iter()) {
        let item = unsafe { std::slice::from_raw_parts(*ptr, *len) };

        if tx.try_send(item.to_vec()).is_err() {
            std::mem::forget(tx);
            return 0;
        }
    }

    std::mem::forget(tx);

    1
}

fn convert_socketaddr(s: &SocketAddr) -> libc::sockaddr {
    let s = InetAddr::from_std(s);
    let s = SockAddr::new_inet(s);
    unsafe { *s.as_ffi_pair().0 }
}

impl OpenDht {
    /// Starts a new DHT client.
    ///
    /// # Arguments
    ///
    /// * `port` - UDP port to use for network communication
    pub fn new(port: u16) -> std::io::Result<OpenDht> {
        let ptr = unsafe { sys::dht_init() };

        if unsafe { sys::dht_run(ptr, port) } != 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(OpenDht(ptr))
    }

    /// Connect this DHT client to other nodes.
    ///
    /// # Remarks
    ///
    /// Usually you should use `bootstrap.ring.cx` on port `4222` here.
    /// You MUST call bootstrap to make your DHT client to work (unless you are
    /// running a bootstrapping node - for experts only).
    pub fn bootstrap(&self, sockets: &[SocketAddr]) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        let tx = Box::new(tx);
        let tx = Box::into_raw(tx);

        let socks: Vec<libc::sockaddr>;
        socks = sockets.iter().map(convert_socketaddr).collect();

        let ptr = socks.as_ptr();

        unsafe {
            sys::dht_bootstrap(self.0, ptr, socks.len(), done_callback, tx);
        }

        rx
    }

    fn loop_(&self) -> Duration {
        let next = unsafe { sys::dht_loop_ms(self.0) };
        Duration::from_millis(next as _)
    }

    fn is_running(&self) -> bool {
        unsafe { sys::dht_is_running(self.0) != 0 }
    }

    /// Wait for DHT threads to end. Run this before your program ends.
    pub fn join(&self) {
        unsafe { sys::dht_join(self.0) };
    }

    /// Put a value on the DHT.
    ///
    /// # Arguments
    ///
    /// * `key` - Key that is used to find the value by the other DHT clients.
    /// * `value` - Value to store on the DHT.
    pub fn put<K: Into<InfoHash>>(&self, key: K, value: &[u8]) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        let tx = Box::new(tx);
        let tx = Box::into_raw(tx);

        let key: InfoHash = key.into();
        let key_ptr = key.as_ptr();
        let ptr = value.as_ptr();

        unsafe {
            sys::dht_put(
                self.0,
                key_ptr,
                key.len(),
                ptr,
                value.len(),
                done_callback,
                tx,
            );
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
    pub fn get<K: Into<InfoHash>>(&self, key: K) -> mpsc::Receiver<Vec<u8>> {
        extern "C" fn get_done_callback(_success: sys::c_bool, tx: *mut mpsc::Sender<Vec<u8>>) {
            let tx = unsafe { Box::from_raw(tx) };
            drop(tx);
        }

        let (get_tx, get_rx) = mpsc::channel(10);
        let get_tx = Box::new(get_tx);
        let get_tx = Box::into_raw(get_tx);

        let key = key.into();
        let key_ptr = key.as_ptr();

        unsafe {
            sys::dht_get(
                self.0,
                key_ptr,
                key.len(),
                get_callback,
                get_tx,
                get_done_callback,
                get_tx,
            );
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
    pub fn listen<K: Into<InfoHash>>(&self, key: K) -> mpsc::Receiver<Vec<u8>> {
        let (get_tx, get_rx) = mpsc::channel(10);
        let get_tx = Box::new(get_tx);
        let get_tx = Box::into_raw(get_tx);

        let key = key.into();
        let key_ptr = key.as_ptr();

        unsafe {
            sys::dht_listen(self.0, key_ptr, key.len(), get_callback, get_tx);
        }

        get_rx
    }

    /// Runs maintainance tasks.. Returns when this function should be called
    /// again, so you probably should `clone` this `OpenDht` instance
    /// and call it in a [tokio::spawn](tokio::executor::spawn)ed loop.
    /// Returns None if this loop can stop.
    pub fn tick(&self) -> Option<Instant> {
        if self.is_running() {
            let next = self.loop_();
            Some(Instant::now() + next)
        } else {
            None
        }
    }

    /// Returns a serialized list of dht clients that this client knows.
    /// You can store this to a file when your program ends and `deserialize()`
    /// when your program starts again.
    pub fn serialize(&self) -> Vec<u8> {
        extern "C" fn cb(src: *const libc::c_uchar, len: libc::size_t, dst: *mut Vec<u8>) {
            unsafe {
                let src = std::slice::from_raw_parts(src, len);
                (*dst).copy_from_slice(&src);
            }
        };

        let buf = Box::new(Vec::new());
        let ptr = Box::into_raw(buf);

        let vec = unsafe {
            sys::serialize(self.0, cb, ptr);
            Box::from_raw(ptr)
        };

        *vec
    }

    /// Deserializes a list of dht clients and adds them to the routing table.
    /// (See `serialize()`)
    pub fn deserialize(&self, buf: &[u8]) {
        let ptr = buf.as_ptr();

        unsafe {
            sys::deserialize(self.0, ptr, buf.len());
        }
    }
}

impl Drop for OpenDht {
    fn drop(&mut self) {
        unsafe {
            sys::dht_drop(self.0);
        }
    }
}
