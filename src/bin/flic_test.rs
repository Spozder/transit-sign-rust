use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

// Constants from the Flic protocol
const CMD_GET_INFO: u8 = 0x01;
const EVT_GET_INFO_RESPONSE: u8 = 0x16;

// Helper function to print bytes
fn format_bytes(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(" ")
}

fn main() -> io::Result<()> {
    println!("Connecting to Flic daemon at 127.0.0.1:5551");
    
    // Connect using the standard library's TcpStream (not Tokio)
    let mut stream = TcpStream::connect("127.0.0.1:5551")?;
    
    // Set socket options similar to what the C client might be using
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    
    println!("Connected to Flic daemon");
    
    // Create a simple GetInfo command (just one byte)
    let cmd_len: u16 = 1; // Command is just 1 byte
    
    // Create the packet with length prefix
    let mut packet = Vec::with_capacity(3);
    packet.push((cmd_len & 0xff) as u8);  // Low byte first
    packet.push((cmd_len >> 8) as u8);    // High byte second
    packet.push(CMD_GET_INFO);           // Command byte
    
    println!("Sending GetInfo command: {}", format_bytes(&packet));
    
    // Write the command to the socket
    stream.write_all(&packet)?;
    stream.flush()?;
    
    println!("Command sent, waiting for response");
    
    // Read the response length (2 bytes)
    let mut len_buf = [0u8; 2];
    match stream.read_exact(&mut len_buf) {
        Ok(_) => {
            // Calculate the length
            let length = (len_buf[0] as u16) | ((len_buf[1] as u16) << 8);
            println!("Got response length: {} bytes", length);
            
            // Read the payload
            let mut payload = vec![0u8; length as usize];
            match stream.read_exact(&mut payload) {
                Ok(_) => {
                    println!("Got response: {}", format_bytes(&payload));
                    if !payload.is_empty() && payload[0] == EVT_GET_INFO_RESPONSE {
                        println!("Successfully received GetInfo response!");
                    } else {
                        println!("Received response with unexpected opcode");
                    }
                },
                Err(e) => println!("Error reading response payload: {}", e),
            }
        },
        Err(e) => println!("Error reading response length: {}", e),
    }
    
    println!("Done");
    Ok(())
}
