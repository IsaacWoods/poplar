use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures::{future::TryFutureExt, stream::StreamExt};
use std::{
    mem,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::Path,
    str,
    time::Instant,
};
use tokio::{fs, net::UdpSocket, try_join};

// TODO: don't hardcode this
const SERVED_DIRECTORY: &str = "/hdd/poplar/tftp_served";

const OPCODE_READ_REQUEST: u16 = 1;
const OPCODE_WRITE_REQUEST: u16 = 2;
const OPCODE_DATA: u16 = 3;
const OPCODE_ACK: u16 = 4;
const OPCODE_ERROR: u16 = 5;
const OPCODE_OACK: u16 = 6;

#[tokio::main]
async fn main() {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 69)).await.unwrap();

    futures::stream::unfold(socket, |s| async move {
        const REQUEST_PACKET_MAX_SIZE: usize = 512;

        let mut buffer = vec![0u8; REQUEST_PACKET_MAX_SIZE];
        let (size, client_address) = s.recv_from(&mut buffer).await.unwrap();

        match TftpRequest::parse(&buffer[0..size]) {
            Ok(request) => Some(((client_address, Ok(request)), s)),
            Err(_) => {
                println!("Warning: parse error on packet from {}. Ignoring.", client_address);
                return Some(((client_address, Err(())), s));
            }
        }
    })
    .for_each_concurrent(None, |(client_address, request)| async move {
        if let Ok(request) = request {
            let _ = handle_request(client_address, request).await;
        }
    })
    .await;
}

struct TftpRequest {
    pub filename: String,
    pub wants_tsize: bool,
    pub block_size: Option<usize>,
}

impl TftpRequest {
    pub fn parse(mut bytes: &[u8]) -> Result<TftpRequest, ()> {
        let opcode = bytes.read_u16::<BigEndian>().unwrap();
        match opcode {
            OPCODE_READ_REQUEST => {
                /*
                 * A read request.
                 *
                 * Split the remainder of the packet into sections delimited by '\0' characters. Strip the
                 * trailing '\0' so we don't end up with an empty string at the end.
                 */
                let mut parts = bytes[..(bytes.len() - 1)].split(|&c| c == b'\0');

                if parts.clone().count() < 2 {
                    println!("Read request does not include at least a filename and mode! Ignoring packet.");
                    return Err(());
                }

                let filename = str::from_utf8(parts.next().unwrap()).map_err(|_| ())?;
                let mode = str::from_utf8(parts.next().unwrap()).map_err(|_| ())?;
                if mode != "octet" {
                    println!("We don't support modes except 'octet'. Ignoring request.");
                    return Err(());
                }

                let options = parts.collect::<Vec<&[u8]>>();
                let mut block_size = None;
                let mut wants_tsize = false;

                for (key, value) in options.chunks(2).map(|chunk| (chunk[0], chunk[1])) {
                    let key = str::from_utf8(key).unwrap();
                    let value = str::from_utf8(value).unwrap();

                    match key {
                        "blksize" => {
                            block_size = Some(str::parse(value).unwrap());
                        }
                        "tsize" => {
                            assert_eq!(value, "0");
                            wants_tsize = true;
                        }
                        _ => println!("Unrecognised option: {} = {}", key, value),
                    }
                }

                Ok(TftpRequest { filename: filename.to_owned(), wants_tsize, block_size })
            }

            OPCODE_WRITE_REQUEST => {
                println!("We don't support write requests. Ignoring.");
                Err(())
            }

            _ => {
                println!("Unrecognised TFTP packet type: {}", opcode);
                Err(())
            }
        }
    }
}

async fn handle_request(client_address: SocketAddr, request: TftpRequest) -> Result<(), ()> {
    use tokio::io::AsyncReadExt;

    // Make an ephemeral socket to send the data from
    let start_time = Instant::now();
    let mut ephemeral_socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await.unwrap();

    /*
     * Open the file and send an error back if it does not exist.
     */
    let path = Path::new(SERVED_DIRECTORY).join(request.filename.clone());
    let mut file = match fs::File::open(&path).await {
        Ok(file) => file,
        Err(_) => {
            println!(
                "{} requested file '{}', but it does not exist. Sending error packet.",
                client_address, request.filename
            );

            // Send an error packet, saying that the file does not exist
            const ERROR_FILE_NOT_FOUND: u16 = 1;

            let mut error_buffer = [0u8; 5];
            (&mut error_buffer[0..2]).write_u16::<BigEndian>(OPCODE_ERROR).unwrap();
            (&mut error_buffer[2..4]).write_u16::<BigEndian>(ERROR_FILE_NOT_FOUND).unwrap();
            // NOTE: no error message, just terminate with a null byte
            error_buffer[4] = b'\0';
            ephemeral_socket.send_to(&error_buffer[..], client_address).await.unwrap();
            return Err(());
        }
    };

    println!("Sending file '{}' to {}", request.filename, client_address);

    /*
     * If any options were requested as part of the request, first respond with an OACK and wait for it to be
     * acknowledged.
     */
    if request.wants_tsize || request.block_size.is_some() {
        // Send an OACK packet
        let mut oack_buffer = vec![0u8; 2];
        (&mut oack_buffer[0..2]).write_u16::<BigEndian>(OPCODE_OACK).unwrap();
        if let Some(block_size) = request.block_size {
            oack_buffer.extend_from_slice(b"blksize\0");
            oack_buffer.extend_from_slice(format!("{}\0", block_size).as_bytes());
        }
        if request.wants_tsize {
            oack_buffer.extend_from_slice(b"tsize\0");
            oack_buffer.extend_from_slice(format!("{}\0", file.metadata().await.unwrap().len()).as_bytes());
        }

        ephemeral_socket.send_to(&oack_buffer[..], client_address).await.unwrap();

        // To acknowledge an OACK, the client should send an ACK packet with a block number of 0
        receive_ack(&mut ephemeral_socket, 0).await?;
    }

    /*
     * Create the buffer into which the file reads will happen, with the DATA packet opcode (3) and the first
     * block number - block 0.
     */
    const HEADER_SIZE: usize = 4;
    const DEFAULT_BLOCK_SIZE: usize = 512;
    let block_size = request.block_size.unwrap_or(DEFAULT_BLOCK_SIZE);

    let mut read_buffer = vec![0u8; HEADER_SIZE + block_size];
    (&mut read_buffer[0..2]).write_u16::<BigEndian>(OPCODE_DATA).unwrap();
    (&mut read_buffer[2..4]).write_u16::<BigEndian>(0).unwrap();

    let mut num_bytes_read = file.read(&mut read_buffer[HEADER_SIZE..]).await.unwrap();
    let mut send_buffer = read_buffer.clone();
    let mut block_number = 0;

    loop {
        block_number += 1;
        let next_read = file.read(&mut read_buffer[HEADER_SIZE..]).map_err(|_| ());
        let send = send_data_packet(
            block_number,
            &mut send_buffer[0..(HEADER_SIZE + num_bytes_read)],
            &mut ephemeral_socket,
            client_address,
        );
        let (new_num_bytes_read, num_bytes_sent) = try_join!(next_read, send)?;

        if num_bytes_sent != read_buffer.len() {
            break;
        }
        num_bytes_read = new_num_bytes_read;
        mem::swap(&mut read_buffer, &mut send_buffer);
    }

    println!("Transferred {} to {} after {:?}", request.filename, client_address, start_time.elapsed());
    Ok(())
}

/// Returns `Err(())` if an error packet is returned instead of an `ACK`.
async fn send_data_packet(
    block_number: u16,
    send_buffer: &mut [u8],
    socket: &mut UdpSocket,
    client_address: SocketAddr,
) -> Result<usize, ()> {
    (&mut send_buffer[2..4]).write_u16::<BigEndian>(block_number).unwrap();
    let num_bytes_sent = socket.send_to(send_buffer, client_address).await.unwrap();
    receive_ack(socket, block_number).await?;
    Ok(num_bytes_sent)
}

/// Returns `Err(())` if instead of an `ACK`, this returns an error packet
async fn receive_ack(socket: &mut UdpSocket, block_number: u16) -> Result<(), ()> {
    let mut ack_buffer = [0u8; 4];
    socket.recv(&mut ack_buffer).await.unwrap();

    match (&ack_buffer[0..2]).read_u16::<BigEndian>().unwrap() {
        OPCODE_ACK => {
            assert_eq!((&ack_buffer[2..4]).read_u16::<BigEndian>().unwrap(), block_number);
            Ok(())
        }
        OPCODE_ERROR => Err(()),
        opcode => {
            println!("Received unexpected opcode: {}", opcode);
            Err(())
        }
    }
}
