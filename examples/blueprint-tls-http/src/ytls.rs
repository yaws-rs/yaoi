//! yTLS Orbit initialization

use blueprint::Orbit;
use blueprint_ytls::{Alpn, TlsServerCtxConfig, TlsServerOrbit};
use blueprint_ytls::{CryptoConfig, CryptoRng};

use super::{CA, CERT, KEY};

pub(super) struct TlsServerConfig {
    ca_cert: Vec<u8>,
    server_cert: Vec<u8>,
    server_private_key: Vec<u8>,
}

impl TlsServerCtxConfig for TlsServerConfig {
    #[inline]
    fn dns_host_name(&self, host: &str) -> bool {
        host == "test.rustcryp.to"
    }
    #[inline]
    fn alpn<'r>(&self, alpn: Alpn<'r>) -> bool {
        if alpn == Alpn::Http11 {
            return true;
        }
        false
    }
    #[inline]
    fn server_private_key(&self) -> &[u8] {
        &self.server_private_key
    }
    #[inline]
    fn server_cert_chain(&self) -> &[u8] {
        &[0, 1]
    }
    #[inline]
    fn server_cert(&self, id: u8) -> &[u8] {
        match id {
            0 => &self.server_cert,
            1 => &self.ca_cert,
            _ => unreachable!(),
        }
    }
}

impl Default for TlsServerConfig {
    fn default() -> TlsServerConfig {
        let ca_vec = load_pem_vec(CA);
        let cert_vec = load_pem_vec(CERT);
        let key_vec = load_pem_vec(KEY);

        let (_cert_type_label, cert_data) = pem_rfc7468::decode_vec(&cert_vec).unwrap();
        let (_key_type_label, key_data_der) = pem_rfc7468::decode_vec(&key_vec).unwrap();
        use sec1::EcPrivateKey;
        let key_info = EcPrivateKey::try_from(key_data_der.as_ref()).unwrap();
        let key_data = key_info.private_key.to_vec();
        let (_ca_type_label, ca_data) = pem_rfc7468::decode_vec(&ca_vec).unwrap();

        Self {
            ca_cert: ca_data,
            server_cert: cert_data,
            server_private_key: key_data,
        }
    }
}

use ytls_rustcrypto::RustCrypto;
use rand::rngs::ThreadRng;

pub fn init_ytls_server() -> TlsServerOrbit<TlsServerConfig, RustCrypto, ThreadRng> {
    let crypto = ytls_rustcrypto::RustCrypto;
    let rng = rand::rng();
    let tls_config_server = TlsServerConfig::default();
    TlsServerOrbit::with_required(tls_config_server, crypto, rng)
}

fn load_pem_vec(path: &str) -> Vec<u8> {
    use std::io::{Read};
    let mut f = std::fs::File::open(path).unwrap();
    let mut data: Vec<u8> = vec![];
    f.read_to_end(&mut data).unwrap();
    data
}

