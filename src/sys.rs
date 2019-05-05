#![allow(non_camel_case_types, improper_ctypes)]

use libc;

use futures::channel::oneshot;
use futures::channel::mpsc;

pub type dht_t = *mut libc::c_void;
pub type c_bool = libc::c_uchar;

#[link(name = "opendht")]
extern {
    pub fn dht_init() -> dht_t;
    pub fn dht_drop(dht: dht_t);
    pub fn dht_join(dht: dht_t);
    pub fn dht_is_running(dht: dht_t) -> libc::c_int;
    pub fn dht_run(dht: dht_t, port: u16) -> libc::c_int;
    pub fn dht_loop_ms(dht: dht_t) -> libc::time_t;

    pub fn dht_bootstrap(dht: dht_t, sa: *const libc::sockaddr,
      sa_count: libc::size_t, done_cb: extern fn(c_bool, *mut oneshot::Sender<bool>),
      done_ptr: *mut oneshot::Sender<bool>);

    pub fn serialize(dht: dht_t, cb: extern fn(*const libc::c_uchar, libc::size_t, *mut Vec<u8>), buf: *mut Vec<u8>);
    pub fn deserialize(dht: dht_t, buf: *const libc::c_uchar, len: libc::size_t);

    pub fn dht_put(dht: dht_t, key: *const libc::c_uchar, key_len: libc::size_t,
        data: *const libc::c_uchar, len: libc::size_t,
        done_cb: extern fn(c_bool, *mut oneshot::Sender<bool>),
        done_ptr: *mut oneshot::Sender<bool>);

    pub fn dht_get(dht: dht_t, key: *const libc::c_uchar, key_len: libc::size_t,
        get_cb: extern fn(*mut *mut libc::c_uchar, *const libc::size_t, libc::size_t, *mut mpsc::Sender<Vec<u8>>) -> c_bool,
        get_tx: *mut mpsc::Sender<Vec<u8>>,
        done_cb: extern fn(c_bool, *mut mpsc::Sender<Vec<u8>>),
        done_tx: *mut mpsc::Sender<Vec<u8>>);

    pub fn dht_listen(dht: dht_t, key: *const libc::c_uchar, key_len: libc::size_t,
        get_cb: extern fn(*mut *mut libc::c_uchar, *const libc::size_t, libc::size_t, *mut mpsc::Sender<Vec<u8>>) -> c_bool,
        get_tx: *mut mpsc::Sender<Vec<u8>>);
}
