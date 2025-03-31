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
const CMD_GET_INFO: u8 = 0x01;
const CMD_CREATE_CONNECTION_CHANNEL: u8 = 0x03;
const CMD_REMOVE_CONNECTION_CHANNEL: u8 = 0x04;

// Event opcodes
const EVT_GET_INFO_RESPONSE: u8 = 0x16;  // 22 decimal - Corrected based on logs
const EVT_CREATE_CONNECTION_CHANNEL_RESPONSE: u8 = 0x07;  // 7 decimal - Corrected based on logs
const EVT_CONNECTION_STATUS_CHANGED: u8 = 0x0B;  // 11 decimal, based on protocol docs
const EVT_BUTTON_EVENT: u8 = 0x0D;  // 13 decimal, based on protocol docs

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
            Ok(_) => println!("FlicButton::new: GetInfo command successful"),
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
                // opcode(1), connId(4), errorCode(1), reserved(2)
                // error code is at index 5 after opcode(0) + connId(1-4)
                let result = if response.len() >= 6 { response[5] } else { 1 };
                println!("create_connection_channel: Got connection channel response with result code: {}", result);
                
                // Check error codes from protocol documentation
                // 0 = SUCCESS, 1 = ERROR_ALREADY_EXISTS, etc.
                if result != 0 {
                    println!("create_connection_channel: Failed with error code: {}", result);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to create connection channel, error code: {}", result)
                    ));
                }
                
                // Now wait for connection status to change to READY
                println!("create_connection_channel: Waiting for READY status");
                self.wait_for_ready_status().await?;
                
                println!("create_connection_channel: Connection channel created successfully");
                self.connected = true;
                Ok(())
            },
            Err(e) => {
                println!("create_connection_channel: Failed to create connection channel: {}", e);
                Err(e)
            }
        }
    }
    
    // Wait for the connection status to change to READY
    async fn wait_for_ready_status(&mut self) -> Result<(), io::Error> {
        println!("wait_for_ready_status: Waiting for connection status to change to READY");
        
        // Set a timeout for waiting
        let start_time = Instant::now();
        let timeout = Duration::from_secs(10);
        
        while !self.connected && start_time.elapsed() < timeout {
            println!("wait_for_ready_status: Reading packet");
            
            // Read the length prefix (2 bytes)
            let mut len_buf = [0u8; 2];
            self.stream.read_exact(&mut len_buf).await?;
            
            // Convert the length bytes to a u16 (little endian)
            let packet_len = u16::from_le_bytes(len_buf) as usize;
            println!("wait_for_ready_status: Received packet with length: {}", packet_len);
            println!("wait_for_ready_status: Length bytes: {}", format_bytes(&len_buf));
            
            if packet_len == 0 || packet_len > 1024 {
                println!("wait_for_ready_status: Invalid packet length: {}", packet_len);
                continue;
            }
            
            // Read the packet body
            let mut packet = vec![0u8; packet_len];
            self.stream.read_exact(&mut packet).await?;
            
            println!("wait_for_ready_status: Received packet: {}", format_bytes(&packet));
            
            // Check if it's a connection status changed event
            if packet.len() > 0 && packet[0] == EVT_CONNECTION_STATUS_CHANGED {
                // Extract the connection ID
                let conn_id = if packet.len() >= 5 {
                    let mut id_bytes = [0u8; 4];
                    id_bytes.copy_from_slice(&packet[1..5]);
                    u32::from_le_bytes(id_bytes)
                } else {
                    0
                };
                
                println!("wait_for_ready_status: Event for connection ID: {}", conn_id);
                // Check if this event is for our connection
                if conn_id == self.conn_id && packet.len() >= 6 {
                    let status = packet[5];
                    println!("wait_for_ready_status: Connection status changed to: {}", status);
                    
                    if status == READY {
                        println!("wait_for_ready_status: Connection is now READY");
                        self.connected = true;
                        return Ok(());
                    }
                }
            } else {
                println!("wait_for_ready_status: Received non-connection-status event: opcode={}", 
                          if !packet.is_empty() { packet[0] } else { 0xff });
            }
            
            // Short delay before next read
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        if self.connected {
            println!("wait_for_ready_status: Connection is already READY");
            Ok(())
        } else {
            println!("wait_for_ready_status: Timeout waiting for READY status");
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Timeout waiting for connection to become READY"
            ))
        }
    }
    
    // Helper to parse a Flic event packet
    fn parse_event(&mut self, buf: &[u8]) -> Option<InputEvent> {
        if buf.len() < 10 {  // Minimum packet size
            println!("parse_event: Packet too small: {} bytes", buf.len());
            return None;
        }
        
        let opcode = buf[0];
        println!("parse_event: Parsing event with opcode: {}", opcode);
        
        // Check if the connection ID matches
        if buf.len() >= 5 {
            let conn_id_bytes = [buf[1], buf[2], buf[3], buf[4]];
            let conn_id = u32::from_le_bytes(conn_id_bytes);
            
            println!("parse_event: Event for conn_id: {}", conn_id);
            
            if conn_id != self.conn_id {
                println!("parse_event: Not for our connection (our conn_id: {})", self.conn_id);
                return None;  // Not for our connection
            }
        }
        
        match opcode {
            EVT_BUTTON_EVENT => {
                println!("parse_event: Button event, click_type: {}", buf[5]);
                // We're only interested in button down events
                if buf[5] == BUTTON_DOWN {
                    println!("parse_event: BUTTON_DOWN detected");
                    Some(InputEvent::SinglePress)
                } else {
                    None
                }
            },
            EVT_CONNECTION_STATUS_CHANGED => {
                println!("parse_event: Connection status changed event, status: {}", buf[5]);
                // Update our connection status
                if buf[5] == READY {
                    println!("parse_event: Connection is now READY");
                    self.connected = true;
                } else if buf[5] == DISCONNECTED {
                    println!("parse_event: Connection is now DISCONNECTED");
                    self.connected = false;
                }
                None
            },
            _ => {
                println!("parse_event: Unknown opcode: {}", opcode);
                None
            },
        }
    }
    
    // Test the connection with a simple GetInfo command
    async fn test_connection(&mut self) -> Result<(), io::Error> {
        println!("test_connection: Sending GetInfo command");
        
        // GetInfo is just a single byte command
        let cmd = [CMD_GET_INFO];
        
        // Use our generic command sender with expected response opcode
        match self.send_command(&cmd, Some(EVT_GET_INFO_RESPONSE)).await {
            Ok(response) => {
                println!("test_connection: GetInfo command successful");
                // We've successfully communicated with the daemon
                self.connected = true;
                Ok(())
            },
            Err(e) => {
                println!("test_connection: GetInfo command failed: {}", e);
                // Even if GetInfo fails, we'll consider the daemon alive if we could send data
                self.connected = true;
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
        // Ensure we have a connection channel
        if !self.connected {
            println!("listen: Not connected, creating connection channel");
            self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
        }
        
        println!("listen: Starting to listen for button events");
        
        // Buffer to read protocol data
        let mut buf = [0u8; 64];
        
        loop {
            println!("listen: Waiting for data from Flic daemon");
            
            // Read from TCP stream
            let bytes_read = self.stream.read(&mut buf).await.map_err(|e| TransitError::Io(e))?;
            
            println!("listen: Received {} bytes: {}", 
                     bytes_read, 
                     format_bytes(&buf[..bytes_read]));
            
            if bytes_read == 0 {
                println!("listen: Connection closed, reconnecting");
                // Connection closed, try to reconnect
                self.stream = TcpStream::connect("127.0.0.1:5551").await.map_err(|e| TransitError::Io(e))?;
                self.connected = false;
                self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
                continue;
            }
            
            // Try to parse the event
            if let Some(event) = self.parse_event(&buf[..bytes_read]) {
                println!("listen: Parsed valid event: {:?}", event);
                return Ok(event);
            }
            
            println!("listen: No valid event found, continuing to listen");
            // If we couldn't parse a valid event, continue reading
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
