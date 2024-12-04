use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub async fn server(address: String) -> Result<(), Box<dyn std::error::Error>> {
    // Bind the server to the specified address
    let listener = TcpListener::bind(&address).await?;
    println!("Server running on {}", address);

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => return, // Connection closed
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("Failed to read from socket: {:?}", e);
                        return;
                    }
                };

                // Echo the data back to the client
                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    eprintln!("Failed to write to socket: {:?}", e);
                    return;
                }
            }
        });
    }
}
