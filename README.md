# yaoi

YAOI or *Y*et *A*nother *O*utput and *I*nput

a batch-first Linux-only io_uring runtime focused on optimal completion with batching.

For a complete HTTPS pipeline example, see [examples/blueprint-tls-http](./examples/blueprint-tls-http).

### Usage

See [API reference](https://docs.rs/yaoi).

Listen for TCP Connections
```ignore
  let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 64, 3)), 8181);
  let listener_strategy = StrategyListener::replenishing(3).fixed_fds(3)
  let mut listener = TcpListener::listen_with_strategy(addr, 256, listener_strategy).unwrap();
  listener
      .set_hugetlb(hugepage::HugePageChoice::HUGE_2MB)
      .unwrap();
```

Set the Listener behaviour upon TCP Accept & Disconnect:
```ignore
   // Setup Listener behaviour on_accept & shutdown
   listener
       .accept_with_cb(&mut bp_listener, |ud, stream, shutdown| {
           let id = stream.fixed_fd().unwrap() as usize;
           stream.run_blueprints(&mut ud[id - 1]).unwrap();
           if shutdown {
               println!("Stream shutting down. Resetting Orbit from Blueprint.");
               ud[id - 1] = ytls_server_blueprints();
           }
       })
       .unwrap();
```

Check-up on any completions peridically
```ignore
  listener.check::<16>(&mut bp_listener).unwrap();
```

### License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion by you, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.