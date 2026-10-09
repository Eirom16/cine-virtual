use std::io::Read;
use tokio::net::TcpListener;

fn read_identity(path: &std::path::Path, max: usize) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "TLS_IDENTITY_LOAD_FAILED")?
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "TLS_IDENTITY_LOAD_FAILED")?;
    if bytes.len() > max {
        return Err("TLS_IDENTITY_TOO_LARGE");
    }
    Ok(bytes)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let startup = cine_server::config::Startup::parse(&args)?;
    if startup.allow_lan && !startup.secure {
        tracing::warn!(
            event = "experimental_lan_enabled",
            warning = "Unencrypted ws, controlled LAN only; not Internet-ready"
        );
    }
    let listener = TcpListener::bind(startup.address).await?;
    tracing::info!(event="server_started",bind=%listener.local_addr()?);
    if startup.secure {
        let tls = match startup.identity_files {
            Some((certificate, key)) => cine_transfer::tls::Identity::from_der(
                read_identity(&certificate, 2048)?,
                read_identity(&key, 8192)?,
            )?,
            None => cine_transfer::tls::Identity::generate_names(vec![
                startup.tls_name,
                startup.address.ip().to_string(),
                "127.0.0.1".into(),
                "localhost".into(),
            ])?,
        };
        println!(
            "{}",
            serde_json::json!({"event":"server_tls","endpoint":format!("{}#tls={}",startup.endpoint,cine_transfer_model::hex(&tls.certificate))})
        );
        cine_server::Server::default()
            .serve_tls(listener, tls.server, async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        return Ok(());
    }
    cine_server::Server::default()
        .serve(listener, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
