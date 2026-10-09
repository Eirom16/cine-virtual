use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let secure = args.iter().any(|s| s == "--tls");
    let allow_lan = args.iter().any(|s| s == "--allow-lan");
    let filtered: Vec<_> = args
        .iter()
        .filter(|s| *s != "--allow-lan" && *s != "--tls")
        .collect();
    let bind = match filtered.as_slice() {
        [] => "127.0.0.1:8765",
        [flag, value] if *flag == "--bind" => value,
        _ => return Err("Usage: cine-server [--bind ADDRESS] [--allow-lan]".into()),
    };
    let address: std::net::SocketAddr = bind.parse()?;
    if !address.ip().is_loopback() && !allow_lan {
        return Err(
            "Non-loopback bind requires --allow-lan; controlled LAN experiment only".into(),
        );
    }
    if allow_lan && !secure {
        tracing::warn!(
            event = "experimental_lan_enabled",
            warning = "Unencrypted ws, controlled LAN only; not Internet-ready"
        );
    }
    let listener = TcpListener::bind(address).await?;
    tracing::info!(event="server_started",bind=%listener.local_addr()?);
    if secure {
        let tls = cine_transfer::tls::Identity::generate_names(vec![
            address.ip().to_string(),
            "127.0.0.1".into(),
            "localhost".into(),
        ])?;
        println!(
            "{}",
            serde_json::json!({"event":"server_tls","endpoint":format!("wss://{address}/#tls={}",cine_transfer_model::hex(&tls.certificate))})
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
