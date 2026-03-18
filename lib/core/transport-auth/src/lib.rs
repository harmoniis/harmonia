use rustls::RootCertStore;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use x509_parser::prelude::{FromDer, X509Certificate};

pub const DEFAULT_TRUST_SCOPE_KEY: &str = "trusted-client-fingerprints-json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedClientIdentity {
    pub identity_fingerprint: String,
    pub cert_fingerprint: String,
}

pub fn normalize_fingerprint(value: &str) -> String {
    value.replace([' ', ':', '-'], "").to_ascii_uppercase()
}

pub fn record_tls_lineage_seed(
    component: &str,
    purpose: &str,
    vault_symbol: &str,
) -> Result<String, String> {
    let _ = harmonia_vault::init_from_env();
    let seed_hex = harmonia_vault::derive_component_seed_hex(component, purpose)?;
    harmonia_vault::set_secret_for_symbol(vault_symbol, &seed_hex)?;
    Ok(seed_hex)
}

pub fn config_path(component: &str, key: &str) -> Result<Option<PathBuf>, String> {
    harmonia_config_store::get_own(component, key)
        .map_err(|e| format!("config-store read failed for {component}/{key}: {e}"))?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .pipe(Ok)
}

pub fn required_config_path(component: &str, key: &str) -> Result<PathBuf, String> {
    config_path(component, key)?
        .ok_or_else(|| format!("missing config-store key {component}/{key}"))
}

pub fn load_trusted_fingerprints(component: &str, key: &str) -> HashSet<String> {
    let raw = harmonia_config_store::get_own(component, key)
        .ok()
        .flatten()
        .unwrap_or_else(|| "[]".to_string());

    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(normalize_fingerprint))
            .filter(|item| !item.is_empty())
            .collect(),
        Ok(Value::String(csv)) => csv
            .split(',')
            .map(normalize_fingerprint)
            .filter(|item| !item.is_empty())
            .collect(),
        _ => HashSet::new(),
    }
}

pub fn load_required_bytes(component: &str, key: &str) -> Result<Vec<u8>, String> {
    let path = required_config_path(component, key)?;
    fs::read(&path).map_err(|e| {
        format!(
            "failed reading {component}/{key} from {}: {e}",
            path.display()
        )
    })
}

pub fn load_optional_bytes(component: &str, key: &str) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = config_path(component, key)? else {
        return Ok(None);
    };
    let bytes = fs::read(&path).map_err(|e| {
        format!(
            "failed reading {component}/{key} from {}: {e}",
            path.display()
        )
    })?;
    Ok(Some(bytes))
}

pub fn load_client_auth_from_config_or_vault(
    component: &str,
    cert_key: &str,
    key_key: &str,
    vault_cert_symbol: &str,
    vault_key_symbol: &str,
) -> Result<Option<(Vec<u8>, Vec<u8>)>, String> {
    match (
        load_optional_bytes(component, cert_key)?,
        load_optional_bytes(component, key_key)?,
    ) {
        (Some(cert), Some(key)) => {
            if let Ok(cert_pem) = String::from_utf8(cert.clone()) {
                let _ = harmonia_vault::set_secret_for_symbol(vault_cert_symbol, &cert_pem);
            }
            if let Ok(key_pem) = String::from_utf8(key.clone()) {
                let _ = harmonia_vault::set_secret_for_symbol(vault_key_symbol, &key_pem);
            }
            Ok(Some((cert, key)))
        }
        (None, None) => {
            let cert = harmonia_vault::get_secret_for_component(component, vault_cert_symbol)
                .ok()
                .flatten();
            let key = harmonia_vault::get_secret_for_component(component, vault_key_symbol)
                .ok()
                .flatten();
            Ok(match (cert, key) {
                (Some(cert), Some(key)) => Some((cert.into_bytes(), key.into_bytes())),
                _ => None,
            })
        }
        _ => Err(format!(
            "incomplete client TLS material for {component}: both {cert_key} and {key_key} are required"
        )),
    }
}

pub fn certificate_fingerprint(der: &[u8]) -> String {
    normalize_fingerprint(&hex::encode_upper(Sha256::digest(der)))
}

pub fn identity_fingerprint_from_der(der: &[u8]) -> Result<String, String> {
    let (_, cert) = X509Certificate::from_der(der)
        .map_err(|e| format!("parse client certificate failed: {e}"))?;
    let common_name = cert
        .subject()
        .iter_common_name()
        .find_map(|item| item.as_str().ok().map(|value| value.to_string()))
        .unwrap_or_default();
    let normalized = normalize_fingerprint(&common_name);
    if normalized.is_empty() {
        return Err("client certificate common-name is empty".to_string());
    }
    Ok(normalized)
}

pub fn verify_client_certificate_der(
    der: &[u8],
    trusted: &HashSet<String>,
) -> Result<VerifiedClientIdentity, String> {
    let identity_fingerprint = identity_fingerprint_from_der(der)?;
    if trusted.is_empty() {
        return Err("no trusted client fingerprints configured".to_string());
    }
    if !trusted.contains(&identity_fingerprint) {
        return Err(format!(
            "client identity {} is not in trusted-client-fingerprints-json",
            identity_fingerprint
        ));
    }
    Ok(VerifiedClientIdentity {
        cert_fingerprint: certificate_fingerprint(der),
        identity_fingerprint,
    })
}

pub fn load_cert_chain(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let file = fs::File::open(path)
        .map_err(|e| format!("failed opening certificate {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed parsing certificate {}: {e}", path.display()))?;
    if certs.is_empty() {
        return Err(format!(
            "certificate file {} did not contain any certs",
            path.display()
        ));
    }
    Ok(certs)
}

pub fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, String> {
    let file = fs::File::open(path)
        .map_err(|e| format!("failed opening private key {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    loop {
        match rustls_pemfile::read_one(&mut reader)
            .map_err(|e| format!("failed parsing private key {}: {e}", path.display()))?
        {
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(PrivateKeyDer::Pkcs8(key)),
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(PrivateKeyDer::Pkcs1(key)),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(PrivateKeyDer::Sec1(key)),
            Some(_) => continue,
            None => {
                return Err(format!(
                    "private key file {} did not contain a supported key",
                    path.display()
                ))
            }
        }
    }
}

pub fn load_root_store(path: &Path) -> Result<RootCertStore, String> {
    let mut store = RootCertStore::empty();
    for cert in load_cert_chain(path)? {
        store
            .add(cert)
            .map_err(|e| format!("failed loading root certificate {}: {e}", path.display()))?;
    }
    Ok(store)
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIICvTCCAaUCFFgM8yDkkEx8CMbeJ7GiS45KxFfjMA0GCSqGSIb3DQEBCwUAMBsx
GTAXBgNVBAMMEGhhcm1vbmlhLXRlc3QtY2EwHhcNMjYwMzE3MTUwOTIxWhcNMjcw
MzE3MTUwOTIxWjAbMRkwFwYDVQQDDBBBQkNERUYxMjM0NTY3ODkwMIIBIjANBgkq
hkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAt5DuMpNW8a0CvyiwDUBk6/LNi7urAUCf
Vst5nJAQugqWPmWCbeKqDN2eY4bARIpJQS38TUWwyPdMK6V0zBI0YxBMmki8dQ9M
5dtFjngnhByFrrMIX/zakv4qg/whjLq7gUD74aOPE1cPQBCC0DFaRgHhtMtx0Ipw
4/DF1px8RGlxa4BRH1l+ohfyYi7OUTlWVZQaxOMou7Okj2XCJ4v/eepJ2qSrRa0v
3+D9Q0fIcpeiATQ6wrDJABB/yEz10T+5KOCZqHP7YmReG6a+ua7U/332Z4PuIv2Z
YiUmxkiZX0udOY1le/JmJLcIxMb5azGvoe6riPW8IJQ0NjXXJfXf3wIDAQABMA0G
CSqGSIb3DQEBCwUAA4IBAQBd3ATMl2w+ddODZsgmrmSih+B7McBOzDLggUCbXDl5
/H4o9+5w/5zXcdL1G1TEzw4kOzAvcxXcsOryrHdbUU5AQ/TGbGbhFNdo/RTLIOBk
mICe5KKJlKG9tMS3dmhPbcdEnlFTaijcxSzT5cjz8nyhc8nmMBL2Jbgr1FutQVMG
1TboYFpHlkbJbzZ4PfhEjtBkX5xdgbgVWJb1sv4ZztkEbzhDgLtJFCArEWvC+TGF
onUvRMbiXxVEHnqUUpUsOw+UdLl/BBh0Y2d5tXWisKor6DqU1U/d+EEJ6qmjAIE8
wHD7q++JijPj738MVLN+6uUINs2wTnG9/yT/kpT4zjho
-----END CERTIFICATE-----"#;

    fn client_der() -> Vec<u8> {
        let mut reader = BufReader::new(CLIENT_CERT.as_bytes());
        rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .as_ref()
            .to_vec()
    }

    #[test]
    fn normalizes_fingerprints() {
        assert_eq!(normalize_fingerprint("ab:cd ef-12"), "ABCDEF12");
    }

    #[test]
    fn extracts_identity_from_certificate_common_name() {
        assert_eq!(
            identity_fingerprint_from_der(&client_der()).unwrap(),
            "ABCDEF1234567890"
        );
    }

    #[test]
    fn verifies_trusted_client_certificate() {
        let trusted = HashSet::from([String::from("ABCDEF1234567890")]);
        let verified = verify_client_certificate_der(&client_der(), &trusted).unwrap();
        assert_eq!(verified.identity_fingerprint, "ABCDEF1234567890");
        assert!(!verified.cert_fingerprint.is_empty());
    }
}
