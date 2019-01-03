#include <opendht.h>
#include <ctime>

#define DHT(dht) ((dht::DhtRunner *) dht)

using std::chrono::steady_clock;
using std::chrono::system_clock;

typedef void dht_t;

typedef void (*done_callback)(bool success, void *ptr);
typedef bool (*get_callback)(uint8_t *values[], size_t *values_len, size_t value_count, void *ptr);

std::time_t to_time_t(steady_clock::time_point t) {
    return system_clock::to_time_t(system_clock::now() +
            std::chrono::duration_cast<system_clock::duration>(t - steady_clock::now()));
}

extern "C" dht_t *dht_init() {
  auto x = new dht::DhtRunner();
  return (dht_t *) x;
}

extern "C" int dht_run(dht_t *dht, uint16_t port) {
  try {
    DHT(dht)->run(port, {}, port);
  }
  catch (dht::DhtException &e) {
    return 0xffff;
  }
  return 0;
}

extern "C" void dht_drop(dht_t *dht) {
  delete DHT(dht);
}

extern "C" int dht_is_running(dht_t *dht) {
  return DHT(dht)->isRunning();
}

extern "C" void dht_join(dht_t *dht) {
  DHT(dht)->join();
}

extern "C" uint64_t dht_loop_ms(dht_t *dht) {
//  return to_time_t(DHT(dht)->loop());
  auto now = steady_clock::now();
  auto next = DHT(dht)->loop();
  
  if (now > next) {
    return 0;
  } else {
    auto dt = next - now;
    return std::chrono::duration_cast<std::chrono::milliseconds>(dt).count();
  }
}

extern "C" void dht_bootstrap(dht_t *dht, sockaddr sa[], size_t sa_count, done_callback done_cb, void *done_ptr) {
  auto cb = [done_cb, done_ptr](bool success) {
    done_cb(success, done_ptr);
  };

  std::vector<dht::SockAddr> vec;
  vec.reserve(sa_count);

  for (size_t i = 0; i < sa_count; i++) {
    vec.push_back(dht::SockAddr(&sa[i]));
  }

  DHT(dht)->bootstrap(vec, cb);
}

extern "C" void dht_put(dht_t *dht, uint8_t *key_, size_t key_len,
             uint8_t *data, size_t data_len, done_callback done_cb, void *done_ptr)
{
  dht::InfoHash key = dht::InfoHash::get(key_, key_len);
  std::shared_ptr<dht::Value> value(new dht::Value(data, data_len));

  auto cb = [done_cb, done_ptr](bool success) {
    done_cb(success, done_ptr);
  };

  DHT(dht)->put(key, value, cb);
}

extern "C" void dht_get(dht_t *dht, uint8_t *key_, size_t key_len, get_callback get_cb,
  void *get_ptr, done_callback done_cb, void *done_ptr)
{
  auto done_cb2 = [done_cb, done_ptr](bool success) {
    done_cb(success, done_ptr);
  };

  auto get_cb2 = [get_cb, get_ptr](const std::vector<std::shared_ptr<dht::Value>>& values) {
    std::vector<uint8_t*> pointers;
    std::vector<size_t> value_lengths;

    for (std::shared_ptr<dht::Value> v : values) {
      uint8_t *ptr = v.get()->data.data();

      pointers.push_back(ptr);
      value_lengths.push_back(v.get()->data.size());
    }

    return get_cb(pointers.data(), value_lengths.data(), pointers.size(), get_ptr);
  };

  dht::InfoHash key = dht::InfoHash::get(key_, key_len);
  DHT(dht)->get(key, get_cb2, done_cb2);
}

extern "C" void dht_listen(dht_t *dht, uint8_t *key_, size_t key_len, get_callback get_cb,
  void *get_ptr)
{
  auto get_cb2 = [get_cb, get_ptr](const std::vector<std::shared_ptr<dht::Value>>& values) {
    std::vector<uint8_t*> pointers;
    std::vector<size_t> value_lengths;

    for (std::shared_ptr<dht::Value> v : values) {
      uint8_t *ptr = v.get()->data.data();

      pointers.push_back(ptr);
      value_lengths.push_back(v.get()->data.size());
    }

    return get_cb(pointers.data(), value_lengths.data(), pointers.size(), get_ptr);
  };

  dht::InfoHash key = dht::InfoHash::get(key_, key_len);
  DHT(dht)->listen(key, get_cb2);
}

extern "C" void serialize(dht_t *dht, void (*cb)(const char *, size_t, void *), void *ptr) {
  msgpack::sbuffer sbuf;
  msgpack::pack(sbuf, DHT(dht)->exportNodes());
  cb(sbuf.data(), sbuf.size(), ptr);
}

extern "C" void deserialize(dht_t *dht, const char *buf, size_t len) {
  msgpack::object_handle oh = msgpack::unpack(buf, len);
  auto nodes = oh.get().as<std::vector<dht::NodeExport>>();
  DHT(dht)->bootstrap(nodes);
}
