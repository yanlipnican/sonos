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
                    // println!("Device: \n{}", device?);
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

                let response = Response::from_fixed_bytes(buf).unwrap();

                println!("{:?}", response);

                yield Ok(String::from(std::str::from_utf8(&buf[..read]).unwrap()));
            }
        })
    }

    // HTTP/1.1 200 OK
    // Location: http://10.69.10.148:1120/
    // Cache-Control: max-age=1800
    // Server: Linux/i686 UPnP/1,0 DLNADOC/1.50 LGE WebOS TV/Version 0.9
    // EXT: 
    // USN: uuid:aaa4ea3d-fda6-ca82-001a-d060ed0f6ff1::urn:schemas-upnp-org:device:MediaRenderer:1
    // ST: urn:schemas-upnp-org:device:MediaRenderer:1
    // Date: Mon, 03 Oct 2022 21:15:24 GMT
    // DLNADeviceName.lge.com: %5bLG%5d%20webOS%20TV%20OLED65B9SLA
    #[derive(Debug)]
    struct Response<'a> {
        buf: [u8; 2048],
        st: &'a str,
        location: &'a str,
        usn: &'a str,
        server: &'a str,
    }

    impl<'a> Response<'a> {
        fn from_fixed_bytes(buf: [u8; 2048]) -> core::result::Result<Self, httparse::Error> {
            let mut response = Self {
                buf,
                location: "",
                usn: "",
                st: "",
                server: "",
            };

            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut res = httparse::Response::new(&mut headers[..]);

            res.parse(&response.buf)?;

            if res.code != Some(200) {
                return Err(httparse::Error::Status);
            }

            for header in res.headers {
                match header.name {
                    "LOCATION" => response.location = core::str::from_utf8(header.value).unwrap(),
                    "USN" => response.usn = core::str::from_utf8(header.value).unwrap(),
                    "ST" => response.st = core::str::from_utf8(header.value).unwrap(),
                    "SERVER" => response.server = core::str::from_utf8(header.value).unwrap(),
                    _ => ()
                };
            }

            Ok(response)
        }
    }
}
