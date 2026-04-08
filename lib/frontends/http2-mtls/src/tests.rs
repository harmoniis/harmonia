use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls_pki_types::ServerName;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::model::{now_ms, HttpBody};
use crate::{
    harmonia_frontend_healthcheck, harmonia_frontend_init, harmonia_frontend_poll,
    harmonia_frontend_send, harmonia_frontend_shutdown, harmonia_frontend_version,
};

const COMPONENT: &str = "http2-frontend";

const CA_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDJzCCAg+gAwIBAgIUBRxy9iDE6Q7JHSNQznvI6Fp7woIwDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQaGFybW9uaWEtdGVzdC1jYTAeFw0yNjAzMTcxNTI4NDda
Fw0yNzAzMTcxNTI4NDdaMBsxGTAXBgNVBAMMEGhhcm1vbmlhLXRlc3QtY2EwggEi
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDhUdzyV59SNOZbUGbjCmSQHYKF
TqmNAXv5xfLLKTPlF/8d/pNATJ/+O+2dPkFO17O84fhi0wChrlCUBBwA5aweUMV7
kGUquHu6Z/ZBfNqJbg5NPaxLMDw0lI0J6HzQNKkH5ajqVfLFsI1kgUXh5ziB8/kx
T+LVxmsiU6/g4/czoGSnjNeAa9QqtqmBaYhPHqoOI5fDabIsBUgMbf0ogEoKDmd2
Ip1SLCi0EDPAPyev8E4F8iLCqTSWGXzcQF8xsrFwmeOkKoIDmoHIq88ThgDB+7zG
USRwGoT4Qspr6QjXHeJ2Ilhs/f2yda6GEU+1Ly5BTf4EDY4xy0dYktHzysxvAgMB
AAGjYzBhMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQW
BBSL+da7Qj6+Np3yJYsChFf4ZjintTAfBgNVHSMEGDAWgBSL+da7Qj6+Np3yJYsC
hFf4ZjintTANBgkqhkiG9w0BAQsFAAOCAQEAKMeFTaCASFMmyY34e8BJ34NiZ90d
qmG2jP49mqrQ45yBtjVe+tpB9utwfkCTuUy3UvcMZ3vDXQbbooMG91UPqzPJu1vC
7YJoKfJQbI9iiV1Y02ZEQYdz5tattlK3NBQhk0lm+T+4qujM3Cfbh350F4DwNNK+
WNTKLK6aHdLf/PWxOgMruUlOLwAtfOZB30EISd0zm5wYb0Zr+7B7gq7OaoPsF+eR
bPoH4sZKHTKpdZtDLjK+4fA9svY/tjSyA5R2vFYa/ZCy5OyutsmrOpU5wA7ELEIt
Lo29zUialeHObCra6uopmg7LKrzikDkIAT9SEwtohrCEm3GYZlw6GUJDTw==
-----END CERTIFICATE-----"#;

const SERVER_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDXzCCAkegAwIBAgIUC/O+UKWoQTUiXw1T8edHYc/unmEwDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQaGFybW9uaWEtdGVzdC1jYTAeFw0yNjAzMTcxNTI4NDda
Fw0yNzAzMTcxNTI4NDdaMCAxHjAcBgNVBAMMFWhhcm1vbmlhLWh0dHAyLXNlcnZl
cjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAPG3glSrTgZ814SDaGZD
TVEdm7bxGl8HNIYIw5LeOs7CDiyQM7wLMTfiRaryJBWgG01k6+eL9a/FZ+mw+XgN
qSYfJ2+L9zuL9saVNk2mQCKVCdWYIY9ukcq05bsYBM6IkW4M2Zeaqe0IwfLdQXBE
8soOICwaz/uSjEddMLKpe4LrLThTXSQomwZjE0Y1VJYsAWgiM4eBLhOzS6iOfJTi
xVoREKqUI4n6PfUGenGT/Fx+ng9fbp6YYsK94z54V3ctom89exig1ffWZPgjrHYZ
vbtCvp+Cc5zWJB+fjJrei8sw14/QDue8MKK4s5gMGkGp8CTD8+8u0doz37QTI9OG
Mv0CAwEAAaOBlTCBkjAgBgNVHREEGTAXghVoYXJtb25pYS1odHRwMi1zZXJ2ZXIw
CQYDVR0TBAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEw
HQYDVR0OBBYEFMl9dVxKMszZ/RIjhfdGpspi2s/DMB8GA1UdIwQYMBaAFIv51rtC
Pr42nfIliwKEV/hmOKe1MA0GCSqGSIb3DQEBCwUAA4IBAQDKNir8aGhzq2QHLaZW
T9y4BbHouzZ8zKJgu0zDTP4PIp7VasAWUqkwpfEosht3cCnBsHMhClRI3+82mgWB
a0v9z5b0ymd076f9EGqSBAQruZl80fLAHJFiE7UY/qA5kuLodCee5AI8pm3a2eUb
pghCI3WxFVCezDoxTys3mgRt/m7kkbik++F8KRzGksJDz9/W8iQslJSmt9uzfZu8
VHPiEdnsyudPg24dbaFDBA7+KDCIgRkVKCs0VGZSqogE/YKTxRNCny54D/HuBINS
ukO038AKzlnfZL0X8R6/RKkVubJbd3c7udT+m4b29xv3fw+tXnrN0KvvDsnLZu+l
SlRm
-----END CERTIFICATE-----"#;

const SERVER_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDxt4JUq04GfNeE
g2hmQ01RHZu28RpfBzSGCMOS3jrOwg4skDO8CzE34kWq8iQVoBtNZOvni/WvxWfp
sPl4DakmHydvi/c7i/bGlTZNpkAilQnVmCGPbpHKtOW7GATOiJFuDNmXmqntCMHy
3UFwRPLKDiAsGs/7koxHXTCyqXuC6y04U10kKJsGYxNGNVSWLAFoIjOHgS4Ts0uo
jnyU4sVaERCqlCOJ+j31Bnpxk/xcfp4PX26emGLCveM+eFd3LaJvPXsYoNX31mT4
I6x2Gb27Qr6fgnOc1iQfn4ya3ovLMNeP0A7nvDCiuLOYDBpBqfAkw/PvLtHaM9+0
EyPThjL9AgMBAAECggEAXENvKJFwyXIqs4aTOYGUCBPUpZpXNhGif0zmFe/ko5oX
3fO3A56EDXA9pnghxO1lrn+IukvGnm6r8NwgBS61s3rtyxqyZpTQv9EhtrbwQSMB
a3nTyZNra+Pr0qPi5dDkLg0Sm1cqaHNQ0LqaqVdwEyccKamcXMr955mPJoshvYD4
So3hk31k/W3IE/xcm4KqH7km0RRSr/4IEnfiGh71UrSI6ryo5UqL/ZeIVWe/yRRA
+UyrNYvaf5qwYkloFlHPw2FVQ2cIiQ8pyMC9RDoWtzgcs0U+tEmHcL2Pyo8WTIAJ
GRm0gPg9QWwpd9yBgBAHDVI8UpyjMQpMF0gDKWIEcwKBgQD7RmKWEibfwHEtU9ti
LExsZdI4UTRjHEhzIJPOz+5VqLgXj1p/K9MGRVxtBBGk8sYwejOvLmaQCgdtG7ax
N2J9QokvvExpMid3WgBLFFgiJHG0OkMRpj9vaLqlnTgvzha8Da6mXMoI2yqh725y
l0H78cCRDbU6NlngfOcPN9M39wKBgQD2Qxy0SaxLy3H+KXHh6xC7ocp3roNm6faG
OhyeNDRLfVIzxzn+Z2ucfHxLHevRWpWeJA6NFNPxuY4a7GcMCoIHrzPuot9Y7u2T
OtXv4CikC0mdRnXPhPEKipUYRl3HUYApXQE7kbU3fysEv6g86u3RtVIOVkRX8acs
3fbgVfR3qwKBgD4FMXA5Kr8vkL/PYuboaDSZLToZUQTlhjxkXhc922XpLwchqwSY
nI1/sUB3MKO2CJUOlJM4sLf8wbh8jqtPMFAajCHsKDAO4Q7keA4QB3Dl7eq+Nq+0
iRPGlcsq8yNZiuL/vYvyeyuUbQFrR6ehDfhRw2YKLCEiKSzvp1hqPwghAoGAPMKu
UGVlF4Zo59b9/EntZP40YHc0gK31X4TzDq2+wWl4YMIlMvn9eSzV1grZ5lu9Url+
xZx/9sJbp5Twj+3/yzmVTKnvBZheEdeQdZEPNfp6/U0nQD6C4qDyzHyAIu+e+ZWy
+imnVrwPtyo6rl0gtH9ScasjTbeYEd/qS8upd+UCgYEA0y/8P4fMFKIbu/Bl7rp3
ezSC8DdtpFkfFn/T8e/PnDm8R2fGbhd73WDHO3oBRSQpwsd6nh8o9zik49+FiQv9
J7GrFcvMzyLEN+oxwv7sUmx9gzPXHLj56KRQdHpwoEWZ77vE6IYn0nMlYEk1kW4W
sWEAkMQW9HBMk1N8aEsfqGQ=
-----END PRIVATE KEY-----"#;

const CLIENT_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDNjCCAh6gAwIBAgIUC/O+UKWoQTUiXw1T8edHYc/unmIwDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQaGFybW9uaWEtdGVzdC1jYTAeFw0yNjAzMTcxNTI4NDda
Fw0yNzAzMTcxNTI4NDdaMBsxGTAXBgNVBAMMEEFCQ0RFRjEyMzQ1Njc4OTAwggEi
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC8s9xcSS8VS6taeILzzmnL0Y66
JvlCHdxZCQRfboQ8pwzg7knn+R1hEVElAaV1md6wHB0chMmYEU6ao4stVXvVkHAT
94jpfHUAdi6TXH+xoiF+q6ICE/PSL7J4x/BAxxi09/XbVwYdnWJYWe4+FZrKg8hH
VIImTuOZoMEn7hCG3JToZF2nqjhY++9FcakLibMFaNq/43At+n+mjy+uqZS5cPsz
WAdO/W+3z5qayZwXgUlr+zOgnvOHfLgZIdiZr3dcOwR2l4IN0ZVhxNmYQj/fE/Rd
hNM3hMdR0Wu8ii85pP3lmNsLSlAhx/gWNxlJvfcKIO6lVp0Aq7iw2aIHpkbrAgMB
AAGjcjBwMAkGA1UdEwQCMAAwDgYDVR0PAQH/BAQDAgWgMBMGA1UdJQQMMAoGCCsG
AQUFBwMCMB0GA1UdDgQWBBQDEh0a24Qpb6Q9EjfOCy3N8MsfWzAfBgNVHSMEGDAW
gBSL+da7Qj6+Np3yJYsChFf4ZjintTANBgkqhkiG9w0BAQsFAAOCAQEAzALnBLwZ
g2F8uXH5HlYvBO9Nw7oyEbKm1tkpsk79D/5/lNEWzgi91W1r2mswGTFTyVBUL6PG
IoDk6Rx24LzjpkOqvCdK+GSTWgLWpYFuI92I1tFFHvX9B4KN2YcK9iUzelf70KHn
1hcgHmE0rKNuEq9SVd8ElRYiwIaT4QGuGI6QBLc65H6vDInUiOpkmRR6ZLBdL6Rf
2RAMbo51+pW/YPq2n/iy0rB4Nz4d/gSYTmPhdAJmHaWkG8DKEjOiotvWU1lQfEyt
MKKEFSuips9a4MiRo5Wy8fD+ZoSJSrT0va954M/JZHr8j8dY1P5e52z4RsGLNhxs
d2fPb1m1emXMZQ==
-----END CERTIFICATE-----"#;

const CLIENT_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC8s9xcSS8VS6ta
eILzzmnL0Y66JvlCHdxZCQRfboQ8pwzg7knn+R1hEVElAaV1md6wHB0chMmYEU6a
o4stVXvVkHAT94jpfHUAdi6TXH+xoiF+q6ICE/PSL7J4x/BAxxi09/XbVwYdnWJY
We4+FZrKg8hHVIImTuOZoMEn7hCG3JToZF2nqjhY++9FcakLibMFaNq/43At+n+m
jy+uqZS5cPszWAdO/W+3z5qayZwXgUlr+zOgnvOHfLgZIdiZr3dcOwR2l4IN0ZVh
xNmYQj/fE/RdhNM3hMdR0Wu8ii85pP3lmNsLSlAhx/gWNxlJvfcKIO6lVp0Aq7iw
2aIHpkbrAgMBAAECggEARs3cBLqnEoACix9Fz5JnSwVV3w5Jn5/Rsoy6Gc6/inyJ
zgpLK+Hivq2/Ozn7af1yu7TIzY8bj1YLHuX3jmqRXQhlrXBHbIh45FPz1PIzraSu
mbdvwgTXi0m/Vyd6Q+wQnrKdixADqPAJWypfROdZXdyFtRIGBba7GsVhRIjEpb0Q
pVuban3x1eCG436lwxVLgO0xeHD0x01q8ia5l2gWsg2S4vM5OhCvFBbh2MkcGRBv
yldR1lmTHJ4BGszKkHrkquKN8oMAFpJ+LYQ5yb0A1DkLk7sBzPXHbNwXMIkGJpzu
CygT2nnLFxwBEd/KSCKXdrROD2b0fWgjCdXTZBzxhQKBgQDeMDPeN/EjfBxq9A6s
n5cMMuC/j14zQeaAThNjtKy9tPF9ProbceSbFgFhLfOjVuq7PJWXA9uOCa6NlZoI
KzBZm+ZSNND1U9tI+jP3WAdjJ8HF0Aw1FgRGoFvl4ppXIjrlAuqZRGcVuWKlib67
vBRVytNliMt1Fbhwk7vxdg8FXQKBgQDZayd1Z6QB1uN0WRMPzxIx05zAWIy4igzy
akCoQwf+ZNbjlTQKDRnRbuSviTciYys9ctnL69UFmRxmGw00pU2eKtwDmVAiWIDN
fAthmmx2YNeURV9IioRtvUkHG2likiIDWwkWspXnPoy5c+sdI5dSamuj2/OJxd81
9UQSKwMw5wKBgE5FbtA2ptUYUK6AwXagVcavWatB5y5pZbkHSB9Us5G032l+onMu
oRjdHKlOVcjRwqkpA42Kh1q3IG2yKOv9wu+eUvncr0vtOY+wzIOy2A9fHwz/aH1+
/wyeSyFlvXc6kMLCT0Ck7yehAhZMuwtJi2RZqjTXhsz9VNcbxBagv1PlAoGBAMDN
gkdd6hXrfvcdSocZZRQkiPwVSm0XlxWd3cqY7szMlbdqB6TmK0ALK+byMp9e++hZ
IgTxFI4LUiDF2ncWI/egIE1ctrBOdaJDX0BllcuAY4xL3IxSsc8zLUCNMW5FEr6R
C0VChyZy1I5c2mGTv0xJrTy4/4Xsn92Uq5HE7OZ/AoGBALPhcm8n4ueOsSoOqDxo
w1DTnJ6L3l8JRBL9AYiMiKozr0U2YOz9wqR2UqCOsY7VEUYbUUNIyLLQt4bU5Gws
X7lSS07bTumHl/boXhEmDxCEgMouZfO2HLHS6EtJkhGe2i5bATsEhnM+kR9W6++c
kfXXAnwXjyrrLvFKUakhMpIQ
-----END PRIVATE KEY-----"#;

fn temp_file(root: &std::path::Path, name: &str, contents: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn poll_once() -> String {
    let mut buffer = vec![0u8; 8192];
    let written = harmonia_frontend_poll(buffer.as_mut_ptr() as *mut c_char, buffer.len());
    if written <= 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buffer[..written as usize]).to_string()
}

fn temp_root(prefix: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now_ms()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

async fn connect_client(
    port: u16,
    ca_path: &std::path::Path,
    client_cert_path: &std::path::Path,
    client_key_path: &std::path::Path,
) -> hyper::client::conn::http2::SendRequest<HttpBody> {
    let tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let root_store = harmonia_transport_auth::load_root_store(ca_path).unwrap();
    let client_certs = harmonia_transport_auth::load_cert_chain(client_cert_path).unwrap();
    let client_key = harmonia_transport_auth::load_private_key(client_key_path).unwrap();

    let mut client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, client_key)
        .unwrap();
    client_config.alpn_protocols = vec![b"h2".to_vec()];
    let connector = TlsConnector::from(Arc::new(client_config));
    let tls = connector
        .connect(ServerName::try_from("harmonia-http2-server").unwrap(), tcp)
        .await
        .unwrap();
    let io = TokioIo::new(tls);
    let (sender, connection) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
        .handshake(io)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    sender
}

#[test]
fn healthcheck_and_version() {
    let _guard = test_lock();
    assert_eq!(harmonia_frontend_healthcheck(), 1);
    let version = unsafe { CStr::from_ptr(harmonia_frontend_version()) }
        .to_str()
        .unwrap();
    assert_eq!(version, "harmonia-http2-mtls/0.1.0");
}

#[test]
fn http2_stream_roundtrip() {
    let _guard = test_lock();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let temp = temp_root("harmonia-http2-mtls-test");
        std::env::set_var("HARMONIA_STATE_ROOT", &temp);
        harmonia_config_store::init_v2().unwrap();

        let port = free_port();
        let bind = format!("127.0.0.1:{port}");
        let ca_path = temp_file(&temp, "ca.crt", CA_CERT);
        let server_cert_path = temp_file(&temp, "server.crt", SERVER_CERT);
        let server_key_path = temp_file(&temp, "server.key", SERVER_KEY);
        let client_cert_path = temp_file(&temp, "client.crt", CLIENT_CERT);
        let client_key_path = temp_file(&temp, "client.key", CLIENT_KEY);

        harmonia_config_store::set_config("harmonia-cli", COMPONENT, "bind", &bind).unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "ca-cert",
            &ca_path.to_string_lossy(),
        )
        .unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "server-cert",
            &server_cert_path.to_string_lossy(),
        )
        .unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "server-key",
            &server_key_path.to_string_lossy(),
        )
        .unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "trusted-client-fingerprints-json",
            "[\"ABCDEF1234567890\"]",
        )
        .unwrap();

        let config = CString::new("()").unwrap();
        assert_eq!(harmonia_frontend_init(config.as_ptr()), 0);
        tokio::time::sleep(Duration::from_millis(150)).await;

        let sender = connect_client(port, &ca_path, &client_cert_path, &client_key_path).await;
        let request = Request::post("/v1/stream/session-1/default")
            .header(CONTENT_TYPE, "application/x-ndjson")
            .body(
                Full::new(Bytes::from_static(b"{\"payload\":\"hello\"}\n"))
                    .map_err(|never| match never {})
                    .boxed(),
            )
            .unwrap();
        let mut response = sender.clone().send_request(request).await.unwrap();

        let mut polled = String::new();
        for _ in 0..20 {
            polled = poll_once();
            if !polled.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(polled.contains("ABCDEF1234567890/session-1/default\thello\t"));
        assert!(polled.contains(":session-id \"session-1\""));
        assert!(polled.contains(":transport-security \"mtls\""));

        let route = CString::new("ABCDEF1234567890/session-1/default").unwrap();
        let payload = CString::new("world").unwrap();
        assert_eq!(harmonia_frontend_send(route.as_ptr(), payload.as_ptr()), 0);

        let body = response.body_mut();
        let mut received = String::new();
        for _ in 0..20 {
            if let Some(frame) = body.frame().await {
                let frame = frame.unwrap();
                if let Some(chunk) = frame.data_ref() {
                    received.push_str(&String::from_utf8_lossy(chunk));
                    break;
                }
            }
        }
        assert!(received.contains("\"payload\":\"world\""));

        assert_eq!(harmonia_frontend_shutdown(), 0);
    });
}

#[test]
fn http2_parallel_sessions_remain_isolated() {
    let _guard = test_lock();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let temp = temp_root("harmonia-http2-mtls-parallel");
        std::env::set_var("HARMONIA_STATE_ROOT", &temp);
        harmonia_config_store::init_v2().unwrap();

        let port = free_port();
        let bind = format!("127.0.0.1:{port}");
        let ca_path = temp_file(&temp, "ca.crt", CA_CERT);
        let server_cert_path = temp_file(&temp, "server.crt", SERVER_CERT);
        let server_key_path = temp_file(&temp, "server.key", SERVER_KEY);
        let client_cert_path = temp_file(&temp, "client.crt", CLIENT_CERT);
        let client_key_path = temp_file(&temp, "client.key", CLIENT_KEY);

        harmonia_config_store::set_config("harmonia-cli", COMPONENT, "bind", &bind).unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "ca-cert",
            &ca_path.to_string_lossy(),
        )
        .unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "server-cert",
            &server_cert_path.to_string_lossy(),
        )
        .unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "server-key",
            &server_key_path.to_string_lossy(),
        )
        .unwrap();
        harmonia_config_store::set_config(
            "harmonia-cli",
            COMPONENT,
            "trusted-client-fingerprints-json",
            "[\"ABCDEF1234567890\"]",
        )
        .unwrap();

        let config = CString::new("()").unwrap();
        assert_eq!(harmonia_frontend_init(config.as_ptr()), 0);
        tokio::time::sleep(Duration::from_millis(150)).await;

        let sender = connect_client(port, &ca_path, &client_cert_path, &client_key_path).await;

        let request_a = Request::post("/v1/stream/session-a/default")
            .header(CONTENT_TYPE, "application/x-ndjson")
            .body(
                Full::new(Bytes::from_static(b"{\"payload\":\"alpha\"}\n"))
                    .map_err(|never| match never {})
                    .boxed(),
            )
            .unwrap();
        let request_b = Request::post("/v1/stream/session-b/alerts")
            .header(CONTENT_TYPE, "application/x-ndjson")
            .body(
                Full::new(Bytes::from_static(b"{\"payload\":\"beta\"}\n"))
                    .map_err(|never| match never {})
                    .boxed(),
            )
            .unwrap();

        let (response_a, response_b) = tokio::join!(
            sender.clone().send_request(request_a),
            sender.clone().send_request(request_b)
        );
        let mut response_a = response_a.unwrap();
        let mut response_b = response_b.unwrap();

        let mut collected = String::new();
        for _ in 0..30 {
            let next = poll_once();
            if !next.is_empty() {
                collected.push_str(&next);
            }
            if collected.contains("ABCDEF1234567890/session-a/default\talpha\t")
                && collected.contains("ABCDEF1234567890/session-b/alerts\tbeta\t")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        assert!(collected.contains("ABCDEF1234567890/session-a/default\talpha\t"));
        assert!(collected.contains("ABCDEF1234567890/session-b/alerts\tbeta\t"));

        let route_a = CString::new("ABCDEF1234567890/session-a/default").unwrap();
        let route_b = CString::new("ABCDEF1234567890/session-b/alerts").unwrap();
        let payload_a = CString::new("reply-a").unwrap();
        let payload_b = CString::new("reply-b").unwrap();
        assert_eq!(
            harmonia_frontend_send(route_a.as_ptr(), payload_a.as_ptr()),
            0
        );
        assert_eq!(
            harmonia_frontend_send(route_b.as_ptr(), payload_b.as_ptr()),
            0
        );

        let body_a = response_a.body_mut();
        let body_b = response_b.body_mut();
        let read_a = async {
            let mut received = String::new();
            for _ in 0..20 {
                if let Some(frame) = body_a.frame().await {
                    let frame = frame.unwrap();
                    if let Some(chunk) = frame.data_ref() {
                        received.push_str(&String::from_utf8_lossy(chunk));
                        break;
                    }
                }
            }
            received
        };
        let read_b = async {
            let mut received = String::new();
            for _ in 0..20 {
                if let Some(frame) = body_b.frame().await {
                    let frame = frame.unwrap();
                    if let Some(chunk) = frame.data_ref() {
                        received.push_str(&String::from_utf8_lossy(chunk));
                        break;
                    }
                }
            }
            received
        };
        let (received_a, received_b) = tokio::join!(read_a, read_b);

        assert!(received_a.contains("\"payload\":\"reply-a\""));
        assert!(received_b.contains("\"payload\":\"reply-b\""));

        assert_eq!(harmonia_frontend_shutdown(), 0);
    });
}
