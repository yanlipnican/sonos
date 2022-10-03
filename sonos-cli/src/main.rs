use futures::{pin_mut, stream::StreamExt};
use clap::{Parser, Subcommand};


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
                let stream = ssdp::discover(ssdp::Builder::default()).await?;
                pin_mut!(stream);

                while let Some(device) = stream.next().await {
                    println!("Device: \n{}", device?);
                }

                Ok(())
            })
        }
    }
}

mod ssdp {
    use futures::stream::Stream;

    pub struct Builder {
        timeout: std::time::Duration,
    }

    impl Default for Builder {
        fn default() -> Self {
            Self { timeout: std::time::Duration::from_secs(1) }
        }
    }

    impl Builder {
        pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
            self.timeout = timeout;
            self
        }

        pub async fn build(self) -> std::io::Result<impl Stream<Item = std::io::Result<String>>> {
            discover(self).await
        }
    }

    pub async fn discover(options: Builder) -> std::io::Result<impl Stream<Item = std::io::Result<String>>> {
        let socket = async_std::net::UdpSocket::bind("0.0.0.0:0").await?;

        let broadcast_addr = "239.255.255.250:1900";

        // let search_target = "urn:schemas-upnp-org:device:ZonePlayer:1";
        let search_target = "ssdp:all";
        let mx = 2;

        let msg = format!(
            "M-SEARCH * HTTP/1.1\r\nHost:{}\r\nMan:\"ssdp:discover\"\r\nMX: {}\r\nST: {}\r\n\r\n",
            broadcast_addr, mx, search_target
        );

        async_std::io::timeout(options.timeout, socket.send_to(msg.as_bytes(), broadcast_addr)).await?;

        Ok(async_stream::stream! {
            loop {
                let mut buf = [0u8; 2048];

                let read = match async_std::io::timeout(options.timeout, socket.recv(&mut buf)).await {
                    Ok(r) => r,
                    Err(err) if err.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(err) => { yield Err(err); break },
                };

                yield Ok(String::from(std::str::from_utf8(&buf[..read]).unwrap()));
            }
        })
    }

}
