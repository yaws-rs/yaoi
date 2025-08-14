use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use yaoi::strategy::StrategyListener;
use yaoi::TcpClientPool;
use yaoi::TcpListener;

use std::thread::sleep;
use std::time::Duration;

struct ConnectInfo;
struct AcceptInfo;

fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8181);

    let listener_strategy = StrategyListener::replenishing(16).fixed_fds(30);

    let mut listener = TcpListener::listen_with_strategy(addr, 16, listener_strategy).unwrap();

    let mut client_pool = TcpClientPool::with_capacity(16).unwrap();

    let mut ud_connect = ConnectInfo;
    let mut ud_serve = ConnectInfo;

    let remaining_connects = client_pool
        .connect_with_cb(addr.clone(), &mut ud_connect, |ud, stream| {
            println!("Client/Stream {:?} connected", stream);
        })
        .unwrap();

    loop {
        let mut ud_accept = AcceptInfo;

        client_pool.check::<32>(&mut ud_connect).unwrap();

        listener
            .accept_with_cb(&mut ud_accept, |u, fno_res, opt_sa| {
                println!("Accepted FileNo {}, Peer Address => {:?}", fno_res, opt_sa);
            })
            .unwrap();

        sleep(Duration::new(1, 0));
    }
}
