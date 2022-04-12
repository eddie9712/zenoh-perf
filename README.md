# zenoh-perf
Rust code for testing and validating zenoh

**Note** that this tool is under its way to support zenoh 0.6.0. 
Currently, ready programs include:
* throughput
  * z_put_thr and z_sub_thr
  * t_pub_thr, t_sub_thr, t_pubsub_thr and t_router_thr
  * r_pub_thr and r_sub_thr
* latency
  * z_ping and z_pong

**Compilation** for the ready programs
```
cargo build --release \
  --bin z_put_thr --bin z_sub_thr \
  --bin t_pub_thr --bin t_sub_thr \
  --bin t_pubsub_thr --bin t_router_thr \
  --bin r_pub_thr --bin r_sub_thr \
  --bin z_ping --bin z_pong
```
