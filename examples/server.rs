//n.b. these features aren't needed to build the library, only the example.
// the library can be build on stable :)
#![feature(let_chains, result_flattening)]
use simple_tftp::{packet, server::*};
use std::net::{IpAddr, Ipv4Addr};

// the ip-address this server should bind too. Only tested with IPv4 but IPv6 should work too.
// The server will always bind to port 69, as required by the spec. If you're testing with a piece of hardware
//  make sure you configure your DHCP server (likely your router) to tell the client about this
// servers existance via DHCP option 66: TFTP Server Name.
const SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1));
//The folder whose contents will be exposed by the server. Any request for a file such as `/hello/world.txt`
// will be appended to this path.
//
//Should be a &Path, but &Path can't be constructed with a const-fn
const FOLDER: &str = "C:\\dev\\tftp-server-test\\boot";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local_path = std::path::Path::new(FOLDER);
    // creates a TFTP server bound to SERVER_IP:69.
    let mut server = Server::connect(SERVER_IP)?;
    loop {
        // every transaction should start with Request packet being send from the client to the server, over UDP, using port 69 for the server
        // and a random port for the client. (CLIENT_IP:P1 -> SERVER_IP:69)
        // The server will then respond over UDP with a Data packet (for read), an Ack packet (for write), or an Error packet
        // using the adress it just got from the client, and picking a new random port for itself.  (SERVER_IP:P2 -> CLIENT_IP:P1)
        let (request, client_addr) = server.get_next_request_from()?;
        // A request can be a read (the most common use case) or a write
        if request.is_read() {
            //and every request comes with a path.
            // NOTE: the TFTP RFC does not actually specify any format for this path. The request might be formatted using Linux path syntax while
            // while the server is using Windows.
            // Joining a nix-style path to a Windows path is generally fine. The reverse is not.
            // some clients preface a request with a leading "/", which will cause join to fail,
            // so we (repeatedly) strip any leading "/" with `trim_start_matches` before joining
            let requested_path = request.filename.trim_start_matches("/");
            println!("[{client_addr}] requested {requested_path:?}");
            // Then we join and canonicalize the path to remove any symlinks, like "../../"
            let full_path = local_path.join(&requested_path).canonicalize();
            // and see if the generated path escapes the folder we're serving. This is not TFTP specific
            // but good practice whenever hosting files :)
            let checked_for_escape = full_path
                .map(|path| {
                    if !path.starts_with(local_path) {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "File not found",
                        ))
                    } else {
                        std::fs::File::open(path)
                    }
                })
                .flatten();

            if let Ok(file) = checked_for_escape {
                //needed to please the borrow checked. requested_path lives in the servers internal UDP receive buffer.
                let requested_path = requested_path.to_owned();
                //we've done all our checks, so now we start sending the file too the client.
                // `create_transfer_to` takes any type that implement std::io::Read, which includes File or vec<u8>.
                // and will buffer it and transfer it to the client in chunks of 512 bytes (as per spec)
                let transfer = server.create_transfer_to(client_addr, file)?;
                //spawn this transfer onto a thread, so that multiple transfers can be handled at once.
                std::thread::spawn(move || {
                    //we need to call finish to actually start transfering the data to the client.
                    // this can error out
                    if let Err(e) = transfer.finish() {
                        //an error can occur for three reasons:
                        // -either we have hit an io-error,
                        // -or the client has send us an error packet during the transfer,
                        // -or the client has send us an invalid reply.
                        // the transfer will not automatically send an error back to the client in this case.
                        eprintln!("[{client_addr}] failed to transfer {requested_path:?}: {e:?}");
                    } else {
                        println!("[{client_addr}] send all packets for {requested_path:?}");
                    }
                });
            } else {
                eprintln!(
                    "[{client_addr}] file {requested_path:?} was not found in {local_path:?}."
                );
                server.send_error_to(
                    packet::Error::new(packet::ErrorCode::FILE_NOT_FOUND, "oopsie"),
                    client_addr,
                )?;
            }
        } else {
            eprintln!("[{client_addr}] Write requests are not supported by this server");
            server.send_error_to(
                packet::Error::new(
                    packet::ErrorCode::ILLEGAL_TFTP_OPERATION,
                    "write operations are not supported by this server",
                ),
                client_addr,
            )?;
        }
    }
}
