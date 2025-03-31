use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use std::thread;
use std::os::unix::io::{AsRawFd, RawFd};
use std::mem;
use libc;

// Constants from the Flic protocol
const CMD_GET_INFO: u8 = 0x00;
const EVT_GET_INFO_RESPONSE: u8 = 0x09;

// Helper function to print bytes
fn format_bytes(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(" ")
}

// Function to check if a socket is ready for reading, similar to select() in C
fn socket_ready_for_reading(fd: RawFd, timeout_secs: i32) -> bool {
    // Set up the fd_set for select
    let mut read_fds: libc::fd_set = unsafe { mem::zeroed() };
    unsafe { libc::FD_ZERO(&mut read_fds) };
    unsafe { libc::FD_SET(fd, &mut read_fds) };
    
    // Set up the timeout
    let mut timeout = libc::timeval {
        tv_sec: timeout_secs as libc::time_t,
        tv_usec: 0,
    };
    
    // Call select
    let result = unsafe { 
        libc::select(
            fd + 1, 
            &mut read_fds, 
            std::ptr::null_mut(), 
            std::ptr::null_mut(), 
            &mut timeout
        ) 
    };
    
    // Check if our socket is ready
    if result > 0 {
        // In Rust bindings, FD_ISSET returns a bool directly
        return unsafe { libc::FD_ISSET(fd, &read_fds) };
    }
    
    false
}

fn main() -> io::Result<()> {
    println!("Connecting to Flic daemon at 127.0.0.1:5551");
    
    // Connect using the standard library's TcpStream
    let mut stream = TcpStream::connect("127.0.0.1:5551")?;
    
    // Set socket options
    stream.set_nodelay(true)?;
    
    // IMPORTANT: Explicitly set to blocking mode
    stream.set_nonblocking(false)?;
    
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
    
    // Sleep briefly to give the daemon time to process
    thread::sleep(Duration::from_millis(500));
    
    // Get the raw file descriptor for the socket
    let socket_fd = stream.as_raw_fd();
    
    // Check if the socket is ready for reading (similar to C++ select)
    println!("Waiting for socket to be ready for reading...");
    if socket_ready_for_reading(socket_fd, 5) {
        println!("Socket is ready for reading!");
    } else {
        println!("Socket is not ready for reading after timeout!");
        return Ok(());
    }
    
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
