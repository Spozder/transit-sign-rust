use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io;
use std::error::Error;
use async_trait::async_trait;
use std::time::Duration;
use std::fmt::Write as FmtWrite;
use std::time::Instant;

use super::{InputEvent, InputHandler};
use crate::error::{TransitError, TransitResult};

// Constants for Flic protocol
// Command opcodes
const CMD_GET_INFO: u8 = 0x00;
const CMD_CREATE_CONNECTION_CHANNEL: u8 = 0x03;
const CMD_REMOVE_CONNECTION_CHANNEL: u8 = 0x04;

// Event opcodes
const EVT_GET_INFO_RESPONSE: u8 = 0x09;  // 9 decimal
const EVT_CREATE_CONNECTION_CHANNEL_RESPONSE: u8 = 0x01;  // 1 decimal
const EVT_CONNECTION_STATUS_CHANGED: u8 = 0x0B;  // 11 decimal
const EVT_BUTTON_EVENT: u8 = 0x04;  // 13 decimal

// Click types
const BUTTON_DOWN: u8 = 0x01;
const BUTTON_UP: u8 = 0x00;
const BUTTON_CLICK: u8 = 0x00;
const BUTTON_HOLD: u8 = 0x01;
const BUTTON_SINGLE_CLICK: u8 = 0x00;
const BUTTON_DOUBLE_CLICK: u8 = 0x01;

// Connection status
const DISCONNECTED: u8 = 0x00;
const CONNECTED: u8 = 0x01;
const READY: u8 = 0x02;

// Latency mode
const LATENCY_NORMAL: u8 = 0x00;

// Bluetooth address type (6 bytes)
type BdAddr = [u8; 6];

// Helper function to format bytes for debug output
fn format_bytes(bytes: &[u8]) -> String {
    let mut s = String::new();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

// Flic button implementation
pub struct FlicButton {
    stream: TcpStream,
    conn_id: u32,
    button_addr: BdAddr,
    connected: bool,
}

impl FlicButton {
    // Generic method to send a command and read the response, matching C++ implementation
    async fn send_command(&mut self, cmd: &[u8], expected_resp_opcode: Option<u8>) -> Result<Vec<u8>, io::Error> {
        // Create a buffer with 2 bytes for length + the command bytes
        // This matches the C++ implementation's approach
        let cmd_len = cmd.len() as u16;
        let mut packet = Vec::with_capacity(2 + cmd.len());
        
        // Set length prefix bytes individually to match C++ implementation exactly
        // new_buf[0] = len & 0xff;
        // new_buf[1] = len >> 8;
        packet.push((cmd_len & 0xff) as u8);
        packet.push((cmd_len >> 8) as u8);
        
        // Copy command bytes to the packet
        packet.extend_from_slice(cmd);
        
        println!("send_command: Sending packet: {}", format_bytes(&packet));
        
        // Send the command and flush the stream to ensure it's sent immediately
        // write_all ensures all bytes are written, similar to the C++ loop
        self.stream.write_all(&packet).await?;
        self.stream.flush().await?;
        println!("send_command: Command sent, waiting for response");
        
        // Give the daemon a moment to process our command
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Read the response if one is expected
        if let Some(expected_opcode) = expected_resp_opcode {
            // First, read the length prefix (2 bytes)
            let mut len_bytes = [0u8; 2];
            match tokio::time::timeout(Duration::from_secs(5), self.stream.read_exact(&mut len_bytes)).await {
                Ok(Ok(_)) => {
                    // Successfully read the length prefix
                    let length = u16::from_le_bytes(len_bytes);
                    println!("send_command: Read response length prefix: {} bytes", length);
                    
                    // Now read the actual payload
                    let mut response = vec![0u8; length as usize];
                    match tokio::time::timeout(Duration::from_secs(5), self.stream.read_exact(&mut response)).await {
                        Ok(Ok(_)) => {
                            println!("send_command: Read response payload: {}", format_bytes(&response));
                            
                            // Check if it's the expected response type
                            if !response.is_empty() && response[0] == expected_opcode {
                                println!("send_command: Received expected response type 0x{:02x}", expected_opcode);
                                Ok(response)
                            } else {
                                println!("send_command: Unexpected response opcode: 0x{:02x}, expected: 0x{:02x}", 
                                         if !response.is_empty() { response[0] } else { 0 }, 
                                         expected_opcode);
                                Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!("Unexpected response opcode: 0x{:02x}, expected: 0x{:02x}", 
                                            if !response.is_empty() { response[0] } else { 0 }, 
                                            expected_opcode)
                                ))
                            }
                        },
                        Ok(Err(e)) => {
                            println!("send_command: Error reading response payload: {}", e);
                            Err(e)
                        },
                        Err(_) => {
                            println!("send_command: Timeout reading response payload");
                            Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "Timeout reading response payload from Flic daemon"
                            ))
                        }
                    }
                },
                Ok(Err(e)) => {
                    println!("send_command: Error reading response length prefix: {}", e);
                    Err(e)
                },
                Err(_) => {
                    println!("send_command: Timeout reading response length prefix");
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Timeout reading response length prefix from Flic daemon"
                    ))
                }
            }
        } else {
            // No response expected, just return empty vector
            Ok(Vec::new())
        }
    }

    pub async fn new(button_addr_str: &str) -> Result<Self, io::Error> {
        println!("FlicButton::new: Creating new FlicButton with address {}", button_addr_str);
        
        // Parse the button address (format: "xx:xx:xx:xx:xx:xx")
        let button_addr = Self::parse_bd_addr(button_addr_str)?;
        println!("FlicButton::new: Parsed button address: {}", format_bytes(&button_addr));
        
        // Connect to the Flic daemon
        println!("FlicButton::new: Connecting to Flic daemon at 127.0.0.1:5551");
        let mut stream = TcpStream::connect("127.0.0.1:5551").await?;
        
        // Set TCP_NODELAY to ensure packets are sent immediately
        // This helps prevent buffering that can lead to connection issues
        if let Err(e) = stream.set_nodelay(true) {
            println!("FlicButton::new: Warning - Failed to set TCP_NODELAY: {}", e);
        }
        
        println!("FlicButton::new: Connected to Flic daemon");
        
        // Use a simple connection ID
        let conn_id = 1;
        
        let mut button = Self { 
            stream,
            conn_id,
            button_addr,
            connected: false,
        };
        
        // Test the connection with a simple command
        println!("FlicButton::new: Testing connection with GetInfo command");
        match button.test_connection().await {
            Ok(_) => {
                println!("FlicButton::new: GetInfo command successful");
                
                // Now try to create a connection channel
                println!("FlicButton::new: Creating connection channel");
                match button.create_connection_channel().await {
                    Ok(_) => println!("FlicButton::new: Connection channel created successfully"),
                    Err(e) => println!("FlicButton::new: Failed to create connection channel: {}", e),
                }
            },
            Err(e) => println!("FlicButton::new: GetInfo command failed: {}", e),
        }
        
        Ok(button)
    }
    
    // Parse a Bluetooth address string into bytes
    fn parse_bd_addr(addr_str: &str) -> Result<BdAddr, io::Error> {
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() != 6 {
            println!("parse_bd_addr: Invalid address format: {}", addr_str);
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Bluetooth address must be in format xx:xx:xx:xx:xx:xx"
            ));
        }
        
        let mut addr = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            match u8::from_str_radix(part, 16) {
                Ok(val) => addr[i] = val,
                Err(e) => {
                    println!("parse_bd_addr: Invalid hex value in part {}: {}", i, part);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid hex value in Bluetooth address: {}", part)
                    ));
                }
            }
        }
        
        // Print the parsed address in the way we'll use it
        println!("Parsed BD address as bytes: {}", format_bytes(&addr));
        
        Ok(addr)
    }
    
    // Create a connection channel to the button
    async fn create_connection_channel(&mut self) -> Result<(), io::Error> {
        println!("create_connection_channel: Creating connection channel for button {:?}", format_bytes(&self.button_addr));
        
        // Prepare the command packet
        let mut cmd = Vec::with_capacity(14);
        cmd.push(CMD_CREATE_CONNECTION_CHANNEL);
        
        // Connection ID (4 bytes, little endian)
        cmd.extend_from_slice(&self.conn_id.to_le_bytes());
        
        // Button address (6 bytes) - ensure correct byte order for Bluetooth address
        // Bluetooth addresses in commands are typically sent in little-endian format
        cmd.extend_from_slice(&self.button_addr);
        
        // Latency mode (1 byte)
        cmd.push(LATENCY_NORMAL);
        
        // Auto disconnect time (2 bytes, little endian) - 511 means never disconnect
        cmd.extend_from_slice(&(511u16).to_le_bytes());
        
        // Send the command and expect a response
        match self.send_command(&cmd, Some(EVT_CREATE_CONNECTION_CHANNEL_RESPONSE)).await {
            Ok(response) => {
                // According to protocol docs, EVT_CREATE_CONNECTION_CHANNEL_RESPONSE structure is:
                // opcode(1), connId(4), errorCode(1), connectionStatus(1)
                let result = if response.len() >= 6 { response[5] } else { 1 };
                let conn_status = if response.len() >= 7 { response[6] } else { DISCONNECTED };
                println!("create_connection_channel: Got connection channel response with result code: {}, connection status: {}", 
                         result, conn_status);
                
                // Check error codes from protocol documentation
                // 0 = SUCCESS, 1 = ERROR_ALREADY_EXISTS, etc.
                if result != 0 {
                    println!("create_connection_channel: Failed with error code: {}", result);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to create connection channel, error code: {}", result)
                    ));
                }
                
                // Connection channel created successfully
                // Note: We do NOT set self.connected = true here because the connection
                // starts in Disconnected state. The listen method will handle the 
                // connection status change events and set connected = true when ready.
                println!("create_connection_channel: Connection channel created successfully (initial status: {})", conn_status);
                Ok(())
            },
            Err(e) => {
                println!("create_connection_channel: Failed to create connection channel: {}", e);
                Err(e)
            }
        }
    }
    
    // Helper to parse a Flic event packet
    fn parse_event(&mut self, buf: &[u8]) -> Option<InputEvent> {
        // Check for minimum viable packet size (opcode + conn_id)
        if buf.len() < 5 {
            println!("parse_event: Packet too small: {} bytes", buf.len());
            return None;
        }
        
        let opcode = buf[0];
        println!("parse_event: Parsing event with opcode: 0x{:02x} ({})", opcode, opcode);
        
        // Check if the connection ID matches
        let conn_id_bytes = [buf[1], buf[2], buf[3], buf[4]];
        let conn_id = u32::from_le_bytes(conn_id_bytes);
        
        println!("parse_event: Event for conn_id: {} (our conn_id: {})", conn_id, self.conn_id);
        
        if conn_id != self.conn_id {
            println!("parse_event: Not for our connection");
            return None;  // Not for our connection
        }
        
        match opcode {
            EVT_BUTTON_EVENT => {
                if buf.len() < 6 {
                    println!("parse_event: Button event packet too small");
                    return None;
                }
                
                let click_type = buf[5];
                println!("parse_event: Button event, click_type: {}", click_type);
                
                // We're interested in all button events
                match click_type {
                    BUTTON_DOWN => {
                        println!("parse_event: BUTTON_DOWN detected");
                        Some(InputEvent::SinglePress)
                    },
                    BUTTON_UP => {
                        println!("parse_event: BUTTON_UP detected - ignoring");
                        None
                    },
                    _ => {
                        println!("parse_event: Unknown click type: {}", click_type);
                        None
                    }
                }
            },
            EVT_CONNECTION_STATUS_CHANGED => {
                if buf.len() < 6 {
                    println!("parse_event: Status change packet too small");
                    return None;
                }
                
                let status = buf[5];
                println!("parse_event: Connection status changed event, status: {}", status);
                
                // Update our connection status
                if status == READY {
                    println!("parse_event: Connection is now READY");
                    self.connected = true;
                } else if status == DISCONNECTED {
                    println!("parse_event: Connection is now DISCONNECTED");
                    self.connected = false;
                }
                None
            },
            _ => {
                println!("parse_event: Unknown or unhandled opcode: 0x{:02x}", opcode);
                None
            },
        }
    }
    
    // Test the connection with a simple GetInfo command
    async fn test_connection(&mut self) -> Result<(), io::Error> {
        println!("test_connection: Sending GetInfo command");
        
        // Using low-level approach for debugging clarity
        println!("test_connection: Using low-level approach for debugging");
        
        // Create the GetInfo command packet
        let cmd = [CMD_GET_INFO, 0x00, 0x00];
        println!("test_connection: Raw bytes to send: {}", format_bytes(&cmd));
                
        // Send it as raw bytes with 2-byte length prefix
        let cmd_len = cmd.len() as u16;
        let mut packet = Vec::with_capacity(2 + cmd.len());
        packet.push((cmd_len & 0xff) as u8);
        packet.push((cmd_len >> 8) as u8);
        packet.extend_from_slice(&cmd);
        
        self.stream.write_all(&packet).await?;
        self.stream.flush().await?;
        println!("test_connection: Command sent, starting read loop");
        
        // Try to read anything that comes back (with a timeout)
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Read whatever comes back for diagnostic purposes
        let mut read_buf = [0u8; 1024];
        match tokio::time::timeout(Duration::from_secs(2), self.stream.read(&mut read_buf)).await {
            Ok(Ok(n)) if n > 0 => {
                println!("test_connection: Read {} bytes: {}", n, format_bytes(&read_buf[0..n]));
                // Don't set self.connected here - it should only be set when a connection channel is established
                Ok(())
            },
            Ok(Ok(_)) => {
                println!("test_connection: Read 0 bytes (connection closed by peer)");
                // Don't set self.connected here
                Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Connection closed by peer"))
            },
            Ok(Err(e)) => {
                println!("test_connection: Error reading response: {}", e);
                // Don't set self.connected here
                Err(e)
            },
            Err(_) => {
                println!("test_connection: Timeout reading response");
                // Even if we time out, we'll consider the daemon alive if we could send data
                // Don't set self.connected here - it should only be set when a connection channel is established
                Ok(())
            }
        }
    }
    
    // Remove the connection channel
    async fn remove_connection_channel(&mut self) -> Result<(), io::Error> {
        println!("remove_connection_channel: Removing connection channel {}", self.conn_id);
        
        // Prepare the command packet
        let mut cmd = Vec::with_capacity(5);
        cmd.push(CMD_REMOVE_CONNECTION_CHANNEL);
        
        // Connection ID (4 bytes, little endian)
        cmd.extend_from_slice(&self.conn_id.to_le_bytes());
        
        // No response is expected for remove_connection_channel
        self.send_command(&cmd, None).await?;
        
        println!("remove_connection_channel: Connection channel removed");
        self.connected = false;
        Ok(())
    }
}

#[async_trait]
impl InputHandler for FlicButton {
    async fn listen(&mut self) -> TransitResult<InputEvent> {        
        println!("listen: Starting to listen for button events");
        
        loop {
            println!("listen: Waiting for data from Flic daemon");
            
            // First, read the 2-byte length prefix
            let mut len_bytes = [0u8; 2];
            match self.stream.read_exact(&mut len_bytes).await {
                Ok(_) => {
                    // Convert the length bytes to a u16 (little-endian)
                    let length = u16::from_le_bytes(len_bytes);
                    println!("listen: Read packet length prefix: {} bytes", length);
                    
                    // Now read the exact payload size
                    let mut payload = vec![0u8; length as usize];
                    match self.stream.read_exact(&mut payload).await {
                        Ok(_) => {
                            println!("listen: Read packet payload: {}", format_bytes(&payload));
                            
                            // Try to parse the event
                            if let Some(event) = self.parse_event(&payload) {
                                println!("listen: Parsed valid event: {:?}", event);
                                return Ok(event);
                            }
                            
                            println!("listen: No valid event found, continuing to listen");
                            // If we couldn't parse a valid event, continue reading
                        },
                        Err(e) => {
                            println!("listen: Error reading payload: {}", e);
                            if e.kind() == io::ErrorKind::UnexpectedEof {
                                // Connection closed, try to reconnect
                                println!("listen: Connection closed, reconnecting");
                                self.stream = TcpStream::connect("127.0.0.1:5551").await.map_err(|e| TransitError::Io(e))?;
                                self.connected = false;
                                self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
                            } else {
                                return Err(TransitError::Io(e));
                            }
                        }
                    }
                },
                Err(e) => {
                    println!("listen: Error reading length prefix: {}", e);
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        // Connection closed, try to reconnect
                        println!("listen: Connection closed, reconnecting");
                        self.stream = TcpStream::connect("127.0.0.1:5551").await.map_err(|e| TransitError::Io(e))?;
                        self.connected = false;
                        self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
                    } else {
                        return Err(TransitError::Io(e));
                    }
                }
            }
        }
    }

    async fn cleanup(&mut self) -> TransitResult<()> {
        println!("cleanup: Cleaning up FlicButton");
        
        // Remove connection channel if connected
        if self.connected {
            println!("cleanup: Removing connection channel");
            self.remove_connection_channel().await.map_err(|e| TransitError::Io(e))?;
        }
        
        // Close connection to flicd
        println!("cleanup: Shutting down TCP connection");
        self.stream.shutdown().await.map_err(|e| TransitError::Io(e))?;
        
        println!("cleanup: FlicButton cleanup complete");
        Ok(())
    }
}
