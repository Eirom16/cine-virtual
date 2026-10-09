//! Private configuration over an out-of-band channel, solely for the transport gate.
use cine_transfer::{storage::Partial, tls::Identity, *};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Write,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Serialize, Deserialize)]
struct Config {
    manifest: Manifest,
    credential: Credential,
    certificate: Vec<u8>,
    address: SocketAddr,
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{{\"status\":\"FAIL\",\"code\":\"{e}\"}}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), Error> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("send") if args.len() == 5 => {
            let source = PathBuf::from(&args[1]);
            let mut file = File::open(source)?;
            let identity = cine_local_media::hash_reader(&mut file, None, |_| true)
                .map_err(|_| Error::Identity)?;
            let manifest = Manifest {
                version: 1,
                transfer_id: uuid::Uuid::new_v4(),
                room_id: uuid::Uuid::new_v4(),
                room_epoch: uuid::Uuid::new_v4(),
                host_id: uuid::Uuid::new_v4(),
                authority_revision: 1,
                media_revision: 1,
                media_id: uuid::Uuid::new_v4(),
                display_name: "Authorized test fixture".into(),
                size_bytes: identity.size_bytes,
                chunk_size: CHUNK_SIZE,
                chunk_count: identity.size_bytes.div_ceil(u64::from(CHUNK_SIZE)) as u32,
                sha256: identity.sha256,
                duration_ms: 1000,
                block_hash: "sha256".into(),
            };
            let credential = Credential::new(&manifest, uuid::Uuid::new_v4())?;
            let tls = Identity::generate()?;
            let listener = TcpListener::bind(&args[2])?;
            let address = args[3].parse().map_err(|_| Error::Manifest)?;
            let config = Config {
                manifest: manifest.clone(),
                credential: credential.clone(),
                certificate: tls.certificate,
                address,
            };
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(&args[4])?
                .write_all(&serde_json::to_vec(&config).map_err(|_| Error::Manifest)?)?;
            println!("{{\"state\":\"listening\"}}");
            std::io::stdout().flush()?;
            let auth = Authorization::new(credential, Duration::from_secs(300));
            let (stream, _) = listener.accept()?;
            let start = Instant::now();
            transport::send(
                stream,
                tls.server,
                &manifest,
                &mut file,
                &auth,
                &Control::default(),
                |_| {},
            )?;
            println!(
                "{{\"status\":\"PASS\",\"bytes\":{},\"elapsed_seconds\":{},\"tls\":\"TLS1.3\"}}",
                manifest.size_bytes,
                start.elapsed().as_secs_f64()
            );
        }
        Some("receive") if args.len() == 3 => {
            let bytes = std::fs::read(&args[1])?;
            if bytes.len() > MAX_CONTROL {
                return Err(Error::Manifest);
            }
            let c: Config = serde_json::from_slice(&bytes).map_err(|_| Error::Manifest)?;
            let mut partial = Partial::create(&PathBuf::from(&args[2]), c.manifest)?;
            let start = Instant::now();
            transport::receive(
                c.address,
                &c.certificate,
                &c.credential,
                &mut partial,
                &Control::default(),
                |p| println!("{}", serde_json::to_string(&p).unwrap()),
            )?;
            println!(
                "{{\"status\":\"PASS\",\"bytes\":{},\"sha256_match\":true,\"elapsed_seconds\":{},\"tls\":\"TLS1.3\"}}",
                partial.manifest.size_bytes,
                start.elapsed().as_secs_f64()
            );
        }
        _ => return Err(Error::Manifest),
    }
    Ok(())
}
