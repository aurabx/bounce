use tokio;

use super::tcp_server;

pub async fn spawn(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Use tokio::spawn to run the server on the specified port
    let address = format!("127.0.0.1:{}", port);
    tokio::spawn(async move {
        if let Err(err) = tcp_server::server(address).await {
            eprintln!("TCP server error: {:?}", err);
        }
    });
    Ok(())
}
