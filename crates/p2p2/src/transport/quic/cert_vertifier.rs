use std::{sync::Arc, time::SystemTime};

use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    server::{ClientCertVerified, ClientCertVerifier},
    Certificate, DistinguishedNames, ServerName,
};

use crate::State;

/// TODO
pub(crate) struct ClientCertificateVerifier(Arc<State>);

impl ClientCertificateVerifier {
    pub fn dangerously_new(state: Arc<State>) -> Arc<dyn ClientCertVerifier> {
        Arc::new(Self(state))
    }
}

impl ClientCertVerifier for ClientCertificateVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_root_subjects(&self) -> Option<DistinguishedNames> {
        Some(vec![])
    }

    fn verify_client_cert(
        &self,
        cert: &Certificate,
        _intermediates: &[Certificate],
        _now: SystemTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        // TODO: Verify certificate expiry
        // TODO: Verify certificate algorithms match
        // TODO: Check common name matches expected value -> We gonna hardcode this.

        match self.0.on_cert_verify(cert) {
            true => Ok(ClientCertVerified::assertion()),
            false => Err(rustls::Error::General("todo".into())), // TODO: Can this be caught on remote? If not maybe only use for Dos related blocking
        }
    }
}

/// ServerCertificateVerifier is a custom certificate verifier that is responsible for verifying the server certificate when making a QUIC connection.
/// It is setup to just allow all certificates
pub(crate) struct ServerCertificateVerifier; // TODO: Private this

impl ServerCertificateVerifier {
    // TODO: Private this
    pub(crate) fn dangerously_new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl ServerCertVerifier for ServerCertificateVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        // TODO: Verify certificate expiry
        // TODO: Verify certificate algorithms match

        Ok(ServerCertVerified::assertion())
    }
}
