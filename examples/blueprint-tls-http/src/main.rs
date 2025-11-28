use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use yaoi::strategy::StrategyListener;
use yaoi::{Blueprints, BlueprintsLayers};
use yaoi::TcpClientPool;
use yaoi::TcpListener;

use std::thread::sleep;
use std::time::Duration;

use blueprint::BluePrint;
use blueprint::Orbit;
use blueprint_h11spec::H11SpecServer;
use blueprint_tls::{TlsServer};
use blueprint_tls::{TlsServerConfig};
use blueprints_known::Orbits;

struct ConnectInfo;
struct AcceptInfo;

use std::path::Path;

const CA: &'static str = "../../../tls/blueprint/certs/ca.rsa4096.crt";
//const CA: &'static str = "../../../tls/blueprint/certs/ca.ed25519.crt";
const CERT: &'static str = "../../../tls/blueprint/certs/rustcryp.to.rsa4096.ca_signed.crt";
//const CERT: &'static str = "../../../tls/blueprint/certs/rustcryp.to.ed25519.ca_signed.crt";
const KEY: &'static str = "../../../tls/blueprint/certs/rustcryp.to.rsa4096.key";
//const KEY: &'static str = "../../../tls/blueprint/certs/rustcryp.to.ed25519.key";

fn clear_server_blueprints() -> Blueprints<0, Orbits> {
    BlueprintsLayers::<0>::no_layers()
        .app(Orbits::H11Server(H11SpecServer::with_defaults().unwrap()))
}

fn tls_server_blueprints() -> Blueprints<1, Orbits> {
   let tls_config_server =
        TlsServerConfig::with_certs_and_key_file(Path::new(CA), Path::new(CERT), Path::new(KEY))
            .unwrap();
    let server_context =
        blueprint_tls::TlsContext::Server(TlsServer::with_config(tls_config_server).unwrap());

    BlueprintsLayers::<1>::layers([Orbits::Tls(server_context)])
        .app(Orbits::H11Server(H11SpecServer::with_defaults().unwrap()))
}

fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 64, 3)), 8181);

    let listener_strategy = StrategyListener::replenishing(3).fixed_fds(3);

    let mut listener = TcpListener::listen_with_strategy(addr, 32, listener_strategy).unwrap();
    listener.set_hugetlb(hugepage::HugePageChoice::HUGE_2MB).unwrap();
    
    let mut bp_listener: [Blueprints::<1, Orbits>; 2] = core::array::from_fn(|_| tls_server_blueprints());
//    let mut bp_listener: [Blueprints::<0, Orbits>; 2] = core::array::from_fn(|_| clear_server_blueprints());    

    // Setup Listener behaviour on_accept
    listener
        .accept_with_cb(&mut bp_listener, |ud, stream| {
            let id = stream.fixed_fd().unwrap() as usize;

            stream.run_blueprints(&mut ud[id-1]).unwrap();
        })
        .unwrap();
    
    loop {
        
        listener.check::<16>(&mut bp_listener).unwrap();

    }
}
