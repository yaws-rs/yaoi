use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use yaoi::TcpListener;
use yaoi::TcpPool;
use yaoi::strategy::{StrategyListener, StrategyRegister};

use std::thread::sleep;
use std::time::Duration;

struct ConnectInfo;
struct AcceptInfo;

fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let strategy = StrategyListener::Replenish(StrategyRegister::Regular);
    let mut listener = TcpListener::listen_with_strategy(addr, 16, strategy).unwrap();

    let mut client_pool = TcpPool::with_capacity(16).unwrap();
    let mut server_pool = TcpPool::with_capacity(16).unwrap();

    let mut ud_connect = ConnectInfo;
    let mut ud_serve = ConnectInfo;
    
    let remaining_connects = client_pool.connect_with_cb(&mut ud_connect, |ud, stream| {
        println!("Client/Stream {:?} connected", stream);
    }).unwrap();


    let remaining_serves = server_pool.serve_with_cb(&mut ud_serve, |ud, stream| {
        println!("Server/Stream {:?} connected", stream);
    }).unwrap();    
    
    loop {

        let mut ud_accept = AcceptInfo;
        
        println!("---- SLeep 1 -----");
        listener.accept_with_cb(&mut ud_accept, |u, fno_res, opt_sa| {
            println!("hmmm..? fno_res = {}, opt_sa => {:?}", fno_res, opt_sa);
        }).unwrap();
        sleep(Duration::new(1, 0));        
    }
}
