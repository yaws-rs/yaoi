use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use yaoi::strategy::StrategyListener;
use yaoi::{Blueprints, BlueprintsLayers};
use yaoi::TcpClientPool;
use yaoi::TcpListener;

use std::thread::sleep;
use std::time::Duration;

use blueprint::BluePrint;
use blueprint::Orbit;
use blueprint_tick_tock::TickTock;
use blueprint_tls::{TlsClient, TlsServer};
use blueprint_tls::{TlsClientConfig, TlsServerConfig};
use blueprints_known::Orbits;

struct ConnectInfo;
struct AcceptInfo;

use std::path::Path;

const CA: &'static str = "../../../tls/blueprint/certs/ca.rsa4096.crt";
const CERT: &'static str = "../../../tls/blueprint/certs/rustcryp.to.rsa4096.ca_signed.crt";
const KEY: &'static str = "../../../tls/blueprint/certs/rustcryp.to.rsa4096.key";

fn server_blueprints() -> Blueprints<1, Orbits> {
   let tls_config_server =
        TlsServerConfig::with_certs_and_key_file(Path::new(CA), Path::new(CERT), Path::new(KEY))
            .unwrap();
    let server_context =
        blueprint_tls::TlsContext::Server(TlsServer::with_config(tls_config_server).unwrap());

    BlueprintsLayers::<1>::layers([Orbits::Tls(server_context)])
        .app(Orbits::TickTock(TickTock::with_defaults().unwrap()))
}

fn client_blueprints() -> Blueprints<1, Orbits> {
    let tls_config_client = TlsClientConfig::with_hostname("localhost").unwrap();       
    let client_context =
        blueprint_tls::TlsContext::Client(TlsClient::with_config(tls_config_client).unwrap());

    BlueprintsLayers::<1>::layers([Orbits::Tls(client_context)])
        .app(Orbits::TickTock(TickTock::with_defaults().unwrap()))
}

fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8181);

    let listener_strategy = StrategyListener::replenishing(2).fixed_fds(2);

    let mut listener = TcpListener::listen_with_strategy(addr, 32, listener_strategy).unwrap();
    listener.set_hugetlb(hugepage::HugePageChoice::HUGE_2MB).unwrap();
    
    let mut client_pool = TcpClientPool::with_capacity(1).unwrap();
    client_pool.set_hugetlb(hugepage::HugePageChoice::HUGE_2MB).unwrap();
    
    let mut bp_clients: [Blueprints::<1, Orbits>; 2] = core::array::from_fn(|_| client_blueprints());
    let mut bp_listener: [Blueprints::<1, Orbits>; 2] = core::array::from_fn(|_| server_blueprints());

    // Setup Client behaviour on_connect
    client_pool
        .connect_with_cb(addr.clone(), &mut bp_clients, |ud, stream| {
            let id = stream.fixed_fd().unwrap() as usize;
            
            stream.run_blueprints(&mut ud[id]).unwrap();
        })
        .unwrap();

    // Setup Listener behaviour on_accept
    listener
        .accept_with_cb(&mut bp_listener, |ud, stream| {
            let id = stream.fixed_fd().unwrap() as usize;

            stream.run_blueprints(&mut ud[id]).unwrap();            
        })
        .unwrap();
    
    loop {
        
        client_pool.check::<16>(&mut bp_clients).unwrap();
        listener.check::<16>(&mut bp_listener).unwrap();

    }
}
