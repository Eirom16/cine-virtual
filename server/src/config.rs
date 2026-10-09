//! Operator startup configuration; advertised WSS name is independent of bind.
use std::{net::SocketAddr, path::PathBuf};

pub struct Startup {
    pub address: SocketAddr,
    pub secure: bool,
    pub allow_lan: bool,
    pub endpoint: String,
    pub tls_name: String,
    pub identity_files: Option<(PathBuf, PathBuf)>,
}
impl Startup {
    pub fn parse(args: &[String]) -> Result<Self, &'static str> {
        let mut address = "127.0.0.1:8765".parse().unwrap();
        let mut secure = false;
        let mut allow_lan = false;
        let (mut advertise, mut cert, mut key) = (None, None, None);
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--tls" => secure = true,
                "--allow-lan" => allow_lan = true,
                "--bind" => {
                    address = args
                        .next()
                        .ok_or("MISSING_BIND")?
                        .parse()
                        .map_err(|_| "INVALID_BIND")?
                }
                "--advertise" => advertise = Some(args.next().ok_or("MISSING_ADVERTISE")?.clone()),
                "--tls-cert" => cert = Some(PathBuf::from(args.next().ok_or("MISSING_CERT")?)),
                "--tls-key" => key = Some(PathBuf::from(args.next().ok_or("MISSING_KEY")?)),
                _ => {
                    return Err(
                        "Usage: cine-server [--bind ADDRESS] [--allow-lan] [--tls] [--advertise wss://HOST:PORT/] [--tls-cert CERT.der --tls-key KEY.pkcs8.der]",
                    );
                }
            }
        }
        let address: SocketAddr = address;
        if !address.ip().is_loopback() && !allow_lan {
            return Err("Non-loopback bind requires --allow-lan; controlled experiment only");
        }
        if !secure && (advertise.is_some() || cert.is_some() || key.is_some()) {
            return Err("TLS_REQUIRED");
        }
        let identity_files = match (cert, key) {
            (None, None) => None,
            (Some(cert), Some(key)) => Some((cert, key)),
            _ => return Err("CERT_AND_KEY_REQUIRED"),
        };
        let (endpoint, tls_name) = if let Some(advertise) = advertise {
            let uri: axum::http::Uri = advertise.parse().map_err(|_| "INVALID_ADVERTISE")?;
            let host = uri.host().ok_or("INVALID_ADVERTISE")?;
            let port_valid = uri.authority().is_some_and(|a| {
                let suffix = a.as_str().strip_prefix(host).unwrap_or("?");
                suffix.is_empty()
                    || suffix
                        .strip_prefix(':')
                        .is_some_and(|s| s.parse::<u16>().is_ok_and(|p| p > 0))
            });
            if uri.scheme_str() != Some("wss")
                || uri.path() != "/"
                || uri.query().is_some()
                || advertise.contains(['#', '@'])
                || host.len() > 253
                || !port_valid
            {
                return Err("INVALID_ADVERTISE");
            }
            let name = host.trim_start_matches('[').trim_end_matches(']');
            if name.parse::<std::net::IpAddr>().is_err()
                && !name.split('.').all(|s| {
                    !s.is_empty()
                        && s.len() <= 63
                        && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                        && s.as_bytes()[0].is_ascii_alphanumeric()
                        && s.as_bytes()[s.len() - 1].is_ascii_alphanumeric()
                })
            {
                return Err("INVALID_ADVERTISE");
            }
            (
                format!("wss://{}/", uri.authority().ok_or("INVALID_ADVERTISE")?),
                name.to_owned(),
            )
        } else {
            (format!("wss://{address}/"), address.ip().to_string())
        };
        Ok(Self {
            address,
            secure,
            allow_lan,
            endpoint,
            tls_name,
            identity_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }
    #[test]
    fn advertised_tls_name_and_bind_are_separate_and_legacy_defaults_survive() {
        let c = Startup::parse(&args(
            "--bind 127.0.0.1:1729 --tls --advertise wss://rooms.example.invalid:443/",
        ))
        .unwrap();
        assert_eq!(c.tls_name, "rooms.example.invalid");
        assert_eq!(c.address.port(), 1729);
        assert_eq!(c.endpoint, "wss://rooms.example.invalid:443/");
        let c = Startup::parse(&[]).unwrap();
        assert!(!c.secure);
        assert!(c.address.ip().is_loopback());
        let c = Startup::parse(&args("--tls --advertise wss://[::1]:1740/")).unwrap();
        assert_eq!(c.tls_name, "::1");
    }
    #[test]
    fn unsafe_advertisements_and_incomplete_identity_fail_closed() {
        for endpoint in [
            "ws://localhost/",
            "wss://localhost/path",
            "wss://localhost/?token=x",
            "wss://user@localhost/",
            "wss://localhost/#tls=x",
            "wss://localhost:0/",
            "wss://localhost:65536/",
            "wss://localhost:bad/",
            "wss://-bad.example/",
            "wss://a..b/",
        ] {
            assert!(
                Startup::parse(&args(&format!("--tls --advertise {endpoint}"))).is_err(),
                "{endpoint}"
            );
        }
        for input in [
            "--advertise wss://localhost/",
            "--tls-cert a --tls-key b",
            "--tls --tls-cert a",
            "--tls --tls-key b",
            "--tls --bind 0.0.0.0:1729",
            "--tls --advertise",
        ] {
            assert!(Startup::parse(&args(input)).is_err());
        }
    }
}
