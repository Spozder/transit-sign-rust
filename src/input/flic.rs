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
    pub async fn new(button_addr_str: &str) -> Result<Self, io::Error> {
        println!("FlicButton::new: Creating new FlicButton with address {}", button_addr_str);
        
        // Parse the button address (format: "xx:xx:xx:xx:xx:xx")
        let button_addr = Self::parse_bd_addr(button_addr_str)?;
        println!("FlicButton::new: Parsed button address: {}", format_bytes(&button_addr));
        
        // Connect to the Flic daemon
        println!("FlicButton::new: Connecting to Flic daemon at 127.0.0.1:5551");
        let stream = TcpStream::connect("127.0.0.1:5551").await?;
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
        
        // Button address (6 bytes)
        cmd.extend_from_slice(&self.button_addr);
        
        // Latency mode (1 byte)
        cmd.push(LATENCY_NORMAL);
        
        // Auto disconnect time (2 bytes, little endian) - 511 means never disconnect
        cmd.extend_from_slice(&(511u16).to_le_bytes());
        
        // Add length prefix (2 bytes, little endian)
        let cmd_len = cmd.len() as u16;
        let mut packet = Vec::with_capacity(2 + cmd_len as usize);
        packet.push(cmd_len as u8);
        packet.push((cmd_len >> 8) as u8);
        packet.extend_from_slice(&cmd);
        
        println!("create_connection_channel: Sending command packet with length prefix: {}", format_bytes(&packet));
        
        // Send the command
        self.stream.write_all(&packet).await?;
        println!("create_connection_channel: Command sent, waiting for response");
        
        // Set a read timeout for the stream
        println!("create_connection_channel: Setting read timeout to 5 seconds");
        
        // Wait for response with timeout
        let mut response = [0u8; 64];
        let read_future = self.stream.read(&mut response);
        
        // Create a timeout future
        let timeout_future = tokio::time::sleep(Duration::from_secs(5));
        
        // Race the read and timeout futures
        let bytes_read = tokio::select! {
            result = read_future => {
                match result {
                    Ok(bytes) => {
                        println!("create_connection_channel: Read completed with {} bytes", bytes);
                        bytes
                    },
                    Err(e) => {
                        println!("create_connection_channel: Read error: {}", e);
                        return Err(e);
                    }
                }
            },
            _ = timeout_future => {
                println!("create_connection_channel: Timeout waiting for response from Flic daemon");
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Timeout waiting for response from Flic daemon"
                ));
            }
        };
        
        println!("create_connection_channel: Received {} bytes: {}", 
                 bytes_read, 
                 format_bytes(&response[..bytes_read]));
        
        if bytes_read < 3 {
            println!("create_connection_channel: Incomplete response (only {} bytes)", bytes_read);
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete response from Flic daemon"
            ));
        }
        
        // Check if it's a connection channel response
        if response[0] == EVT_CREATE_CONNECTION_CHANNEL_RESPONSE {
            // Extract connection ID and result
            let result = response[2];
            println!("create_connection_channel: Got connection channel response with result code: {}", result);
            
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
            Ok(())
        } else {
            println!("create_connection_channel: Unexpected response opcode: {}", response[0]);
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected response opcode: {}", response[0])
            ))
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
                if packet.len() >= 6 {
                    let status = packet[5];
                    println!("wait_for_ready_status: Connection status changed to: {}", status);
                    
                    if status == READY {
                        println!("wait_for_ready_status: Connection is now READY");
                        self.connected = true;
                        return Ok(());
                    }
                }
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
        
        // Prepare the command packet - GetInfo is just a single byte
        let cmd = [CMD_GET_INFO];
        
        // Add length prefix (2 bytes, little endian)
        let mut packet = Vec::with_capacity(3);
        packet.push(1); // Length low byte
        packet.push(0); // Length high byte
        packet.push(CMD_GET_INFO);
        
        // Send the command
        self.stream.write_all(&packet).await?;
        println!("test_connection: Command sent, waiting for response");
        
        // Wait for response with timeout
        let mut response = [0u8; 64];
        let read_future = self.stream.read(&mut response);
        
        // Create a timeout future
        let timeout_future = tokio::time::sleep(Duration::from_secs(5));
        
        // Race the read and timeout futures
        let bytes_read = tokio::select! {
            result = read_future => {
                match result {
                    Ok(bytes) => {
                        println!("test_connection: Read completed with {} bytes", bytes);
                        bytes
                    },
                    Err(e) => {
                        println!("test_connection: Read error: {}", e);
                        return Err(e);
                    }
                }
            },
            _ = timeout_future => {
                println!("test_connection: Timeout waiting for response from Flic daemon");
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Timeout waiting for response from Flic daemon"
                ));
            }
        };
        
        println!("test_connection: Received {} bytes: {}", 
                 bytes_read, 
                 format_bytes(&response[..bytes_read]));
        
        if bytes_read == 0 {
            println!("test_connection: Empty response");
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Empty response from Flic daemon"
            ));
        }
        
        // Check if it's a GetInfo response
        if response[0] == EVT_GET_INFO_RESPONSE {
            println!("test_connection: Got GetInfo response (opcode 0x16)");
            Ok(())
        } else {
            println!("test_connection: Unexpected response opcode: {}, expected: {}", response[0], EVT_GET_INFO_RESPONSE);
            // We'll still return Ok here since we got some response
            Ok(())
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
        
        // Add length prefix (2 bytes, little endian)
        let cmd_len = cmd.len() as u16;
        let mut packet = Vec::with_capacity(2 + cmd_len as usize);
        packet.push(cmd_len as u8);
        packet.push((cmd_len >> 8) as u8);
        packet.extend_from_slice(&cmd);
        
        println!("remove_connection_channel: Sending command: {}", format_bytes(&packet));
        
        // Send the command
        self.stream.write_all(&packet).await?;
        
        println!("remove_connection_channel: Connection channel removed");
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
