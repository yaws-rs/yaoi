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
use blueprints_known::Orbits;

use blueprint_rustls::{TlsServer as RustlsServer, TlsServerConfig as RustlsServerConfig};

use blueprint_ytls::{CryptoConfig, CryptoRng};
use blueprint_ytls::{TlsServer, TlsServerConfig, TlsServerCtxConfig};

struct ConnectInfo;
struct AcceptInfo;

use std::path::Path;

//const CA: &'static str = "../../../tls/test_certs/ca.rsa4096.crt";
//const CA: &'static str = "../../../tls/test_certs/ca.ed25519.crt";
const CA: &'static str = "../../../tls/test_certs/ca.prime256v1.crt";

//const CERT: &'static str = "../../../tls/test_certs/rustcryp.to.rsa4096.ca_signed.crt";
//const CERT: &'static str = "../../../tls/test_certs/rustcryp.to.ed25519.ca_signed.crt";
const CERT: &'static str = "../../../tls/test_certs/rustcryp.to.prime256v1.ca_signed.crt";

//const KEY: &'static str = "../../../tls/test_certs/rustcryp.to.rsa4096.key";
//const KEY: &'static str = "../../../tls/test_certs/rustcryp.to.ed25519.key";
const KEY: &'static str = "../../../tls/test_certs/rustcryp.to.prime256v1.pem";

/*
fn clear_server_blueprints() -> Blueprints<0, Orbits> {
    BlueprintsLayers::<0>::no_layers()
        .app(Orbits::H11Server(H11SpecServer::with_defaults().unwrap()))
} */

fn load_pem_vec(path: &str) -> Vec<u8> {
    use std::io::{Read};
    let mut f = std::fs::File::open(path).unwrap();
    let mut data: Vec<u8> = vec![];
    f.read_to_end(&mut data).unwrap();
    data
}


fn ytls_server_blueprints() -> Blueprints<1, Orbits> {

    let ca_vec = load_pem_vec(CA);
    let cert_vec = load_pem_vec(CERT);
    let key_vec = load_pem_vec(KEY);

    let (cert_type_label, cert_data) = pem_rfc7468::decode_vec(&cert_vec).unwrap();
    let (key_type_label, key_data_der) = pem_rfc7468::decode_vec(&key_vec).unwrap();
    use sec1::EcPrivateKey;
    let key_info = EcPrivateKey::try_from(key_data_der.as_ref()).unwrap();
    let key_data = key_info.private_key.to_vec();
    let (ca_type_label, ca_data) = pem_rfc7468::decode_vec(&ca_vec).unwrap();

    let tls_config_server = TlsServerConfig::with_ca_cert_key(&ca_data, &cert_data, &key_data).unwrap();
    
    BlueprintsLayers::<1>::layers([
        Orbits::YtlsServer(TlsServer::with_configuration(tls_config_server).unwrap())
    ])
        .app(Orbits::H11Server(H11SpecServer::with_defaults().unwrap()))
}


fn rustls_server_blueprints() -> Blueprints<1, Orbits> {
   let tls_config_server =
        RustlsServerConfig::with_certs_and_key_file(Path::new(CA), Path::new(CERT), Path::new(KEY))
            .unwrap();
    let server_context =
        blueprint_rustls::TlsContext::Server(RustlsServer::with_config(tls_config_server).unwrap());

    BlueprintsLayers::<1>::layers([Orbits::Rustls(server_context)])
        .app(Orbits::H11Server(H11SpecServer::with_defaults().unwrap()))
}


fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 64, 3)), 8181);

    let listener_strategy = StrategyListener::replenishing(3).fixed_fds(3);

    let mut listener = TcpListener::listen_with_strategy(addr, 256, listener_strategy).unwrap();
    listener.set_hugetlb(hugepage::HugePageChoice::HUGE_2MB).unwrap();
    
    let mut bp_listener: [Blueprints::<1, Orbits>; 2] = core::array::from_fn(|_| ytls_server_blueprints());
//    let mut bp_listener: [Blueprints::<1, Orbits>; 2] = core::array::from_fn(|_| rustls_server_blueprints());    
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
