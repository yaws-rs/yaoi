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

fn client_blueprints() -> Blueprints<1, Orbits> {
    let tls_config_client = TlsClientConfig::with_hostname("localhost").unwrap();       
    let client_context =
        blueprint_tls::TlsContext::Client(TlsClient::with_config(tls_config_client).unwrap());
    let client_blueprints = BlueprintsLayers::<1>::layers([Orbits::Tls(client_context)])
        .app(Orbits::TickTock(TickTock::with_defaults().unwrap()));
    client_blueprints
}

fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8181);

    let listener_strategy = StrategyListener::replenishing(16).fixed_fds(30);

    let mut listener = TcpListener::listen_with_strategy(addr, 16, listener_strategy).unwrap();

    let mut client_pool = TcpClientPool::with_capacity(16).unwrap();
    client_pool.set_hugetlb(hugepage::HugePageChoice::HUGE_2MB).unwrap();
    
    let tls_config_server =
        TlsServerConfig::with_certs_and_key_file(Path::new(CA), Path::new(CERT), Path::new(KEY))
            .unwrap();

    let mut bp_clients: [Blueprints::<1, Orbits>; 16] = core::array::from_fn(|_| client_blueprints());
    let mut ud_connect = ConnectInfo;
    let mut ud_serve = ConnectInfo;

    let remaining_connects = client_pool
        .connect_with_cb(addr.clone(), &mut bp_clients, |ud, stream| {
            let id = stream.fixed_fd().unwrap() as usize;
            
            println!("Client/Stream[{}] {:?} connected", id,stream);

            stream.run_blueprints(&mut ud[id]).unwrap();
        })
        .unwrap();

    loop {
        let mut ud_accept = AcceptInfo;

        client_pool.check::<32>(&mut bp_clients).unwrap();

        listener
            .accept_with_cb(&mut ud_accept, |u, fno_res, opt_sa| {
                println!("Accepted FileNo {}, Peer Address => {:?}", fno_res, opt_sa);
            })
            .unwrap();

        sleep(Duration::from_micros(10_000));
    }
}
