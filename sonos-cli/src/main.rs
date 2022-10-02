use async_std::net::UdpSocket;
use clap::{Parser, Subcommand};
use futures::{
    pin_mut,
    stream::{Stream, StreamExt},
};

#[derive(Debug, Parser)]
#[clap(name = "sonos")]
#[clap(about = "Sonos cli", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Discover,
}

fn main() -> async_std::io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Discover => {
            return async_std::task::block_on(async {
                let stream = SSDP::broadcast(std::time::Duration::from_secs(2))
                    .await?
                    .listen();
                pin_mut!(stream);

                while let Some(device) = stream.next().await {
                    println!("Device: \n{}", device?);
                }

                Ok(())
            })
        }
    }
}

struct SSDP {
    socket: UdpSocket,
    timeout: std::time::Duration,
}

impl SSDP {
    pub async fn broadcast(timeout: std::time::Duration) -> std::io::Result<Self> {
        let socket = async_std::net::UdpSocket::bind("0.0.0.0:0").await?;

        let broadcast_addr = "239.255.255.250:1900";

        // let search_target = "urn:schemas-upnp-org:device:ZonePlayer:1";
        let search_target = "ssdp:all";
        let mx = 2;

        let msg = format!(
            "M-SEARCH * HTTP/1.1\r\nHost:{}\r\nMan:\"ssdp:discover\"\r\nMX: {}\r\nST: {}\r\n\r\n",
            broadcast_addr, mx, search_target
        );

        async_std::io::timeout(timeout, socket.send_to(msg.as_bytes(), broadcast_addr)).await?;

        Ok(Self { socket, timeout })
    }

    async fn next_device(&self) -> Option<std::io::Result<String>> {
        let mut buf = [0u8; 2048];

        let read = match async_std::io::timeout(self.timeout, self.socket.recv(&mut buf)).await {
            Ok(r) => r,
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut => return None,
            Err(err) => return Some(Err(err)),
        };

        Some(Ok(String::from(std::str::from_utf8(&buf[..read]).unwrap())))
    }

    pub fn listen(self) -> impl Stream<Item = std::io::Result<String>> {
        async_stream::stream! {
           let broadcast = self;
           while let Some(device) = broadcast.next_device().await {
              yield device;
           }
        }
    }
}

struct Device {
    name: [u8; 256],
}

impl Device {
    pub fn from_discover(buf: &[u8]) -> Self {
        Self { name: [0u8; 256] }
    }

    pub fn name(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.name) }
    }
}
