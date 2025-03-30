use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io;
use std::error::Error;
use async_trait::async_trait;
use std::time::Duration;

use super::{InputEvent, InputHandler};
use crate::error::{TransitError, TransitResult};

// Constants for Flic protocol
// Command opcodes
const CMD_CREATE_CONNECTION_CHANNEL: u8 = 3;
const CMD_REMOVE_CONNECTION_CHANNEL: u8 = 4;

// Event opcodes
const EVT_CREATE_CONNECTION_CHANNEL_RESPONSE: u8 = 2;
const EVT_CONNECTION_STATUS_CHANGED: u8 = 3;
const EVT_BUTTON_UP_OR_DOWN: u8 = 4;
const EVT_BUTTON_CLICK_OR_HOLD: u8 = 5;
const EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK: u8 = 6;
const EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK_OR_HOLD: u8 = 7;

// Click types
const BUTTON_DOWN: u8 = 0;
const BUTTON_UP: u8 = 1;
const BUTTON_CLICK: u8 = 0;
const BUTTON_HOLD: u8 = 1;
const BUTTON_SINGLE_CLICK: u8 = 0;
const BUTTON_DOUBLE_CLICK: u8 = 1;

// Connection status
const DISCONNECTED: u8 = 0;
const CONNECTED: u8 = 1;
const READY: u8 = 2;

// Latency mode
const LATENCY_NORMAL: u8 = 0;

// Bluetooth address type (6 bytes)
type BdAddr = [u8; 6];

// Flic button implementation
pub struct FlicButton {
    stream: TcpStream,
    conn_id: u32,
    button_addr: BdAddr,
    connected: bool,
}

impl FlicButton {
    pub async fn new(button_addr_str: &str) -> Result<Self, io::Error> {
        // Parse the button address (format: "xx:xx:xx:xx:xx:xx")
        let button_addr = Self::parse_bd_addr(button_addr_str)?;
        
        // Connect to the Flic daemon
        let stream = TcpStream::connect("127.0.0.1:5551").await?;
        
        // Use a simple connection ID
        let conn_id = 1;
        
        Ok(Self { 
            stream,
            conn_id,
            button_addr,
            connected: false,
        })
    }
    
    // Parse a Bluetooth address string into bytes
    fn parse_bd_addr(addr_str: &str) -> Result<BdAddr, io::Error> {
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() != 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Bluetooth address must be in format xx:xx:xx:xx:xx:xx"
            ));
        }
        
        let mut addr = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            addr[i] = u8::from_str_radix(part, 16).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid hex value in Bluetooth address: {}", part)
                )
            })?;
        }
        
        Ok(addr)
    }
    
    // Create a connection channel to the button
    async fn create_connection_channel(&mut self) -> Result<(), io::Error> {
        // Prepare the command packet
        let mut cmd = Vec::with_capacity(16);
        cmd.push(CMD_CREATE_CONNECTION_CHANNEL);
        
        // Connection ID (4 bytes, little endian)
        cmd.extend_from_slice(&self.conn_id.to_le_bytes());
        
        // Button address (6 bytes)
        cmd.extend_from_slice(&self.button_addr);
        
        // Latency mode (1 byte)
        cmd.push(LATENCY_NORMAL);
        
        // Auto disconnect time (2 bytes, little endian) - 511 means never disconnect
        cmd.extend_from_slice(&(511u16).to_le_bytes());
        
        // Send the command
        self.stream.write_all(&cmd).await?;
        
        // Wait for response
        let mut response = [0u8; 64];
        let bytes_read = self.stream.read(&mut response).await?;
        
        if bytes_read < 3 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete response from Flic daemon"
            ));
        }
        
        // Check if it's a connection channel response
        if response[0] == EVT_CREATE_CONNECTION_CHANNEL_RESPONSE {
            // Extract connection ID and result
            let result = response[2];
            if result != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to create connection channel, error code: {}", result)
                ));
            }
            
            // Now wait for connection status to change to READY
            self.wait_for_ready_status().await?;
            
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected response opcode: {}", response[0])
            ))
        }
    }
    
    // Wait for the connection status to change to READY
    async fn wait_for_ready_status(&mut self) -> Result<(), io::Error> {
        let mut buf = [0u8; 64];
        let mut attempts = 0;
        
        while attempts < 10 {
            let bytes_read = self.stream.read(&mut buf).await?;
            
            if bytes_read < 3 {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            
            if buf[0] == EVT_CONNECTION_STATUS_CHANGED {
                // Check if the connection ID matches
                let conn_id_bytes = [buf[1], buf[2], buf[3], buf[4]];
                let conn_id = u32::from_le_bytes(conn_id_bytes);
                
                if conn_id == self.conn_id {
                    let status = buf[5];
                    if status == READY {
                        self.connected = true;
                        return Ok(());
                    } else if status == DISCONNECTED {
                        return Err(io::Error::new(
                            io::ErrorKind::ConnectionAborted,
                            "Button disconnected before ready"
                        ));
                    }
                }
            }
            
            attempts += 1;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Timed out waiting for button to be ready"
        ))
    }
    
    // Helper to parse a Flic event packet
    fn parse_event(&mut self, buf: &[u8]) -> Option<InputEvent> {
        if buf.len() < 10 {  // Minimum packet size
            return None;
        }
        
        let opcode = buf[0];
        
        // Check if the connection ID matches
        if buf.len() >= 5 {
            let conn_id_bytes = [buf[1], buf[2], buf[3], buf[4]];
            let conn_id = u32::from_le_bytes(conn_id_bytes);
            
            if conn_id != self.conn_id {
                return None;  // Not for our connection
            }
        }
        
        match opcode {
            EVT_BUTTON_UP_OR_DOWN => {
                // We're only interested in button down events
                if buf[5] == BUTTON_DOWN {
                    Some(InputEvent::SinglePress)
                } else {
                    None
                }
            },
            EVT_BUTTON_CLICK_OR_HOLD => {
                match buf[5] {
                    BUTTON_CLICK => Some(InputEvent::SinglePress),
                    BUTTON_HOLD => Some(InputEvent::LongPress),
                    _ => None,
                }
            },
            EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK => {
                match buf[5] {
                    BUTTON_SINGLE_CLICK => Some(InputEvent::SinglePress),
                    BUTTON_DOUBLE_CLICK => Some(InputEvent::DoublePress),
                    _ => None,
                }
            },
            EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK_OR_HOLD => {
                match buf[5] {
                    BUTTON_SINGLE_CLICK => Some(InputEvent::SinglePress),
                    BUTTON_DOUBLE_CLICK => Some(InputEvent::DoublePress),
                    BUTTON_HOLD => Some(InputEvent::LongPress),
                    _ => None,
                }
            },
            EVT_CONNECTION_STATUS_CHANGED => {
                // Update our connection status
                if buf[5] == READY {
                    self.connected = true;
                } else if buf[5] == DISCONNECTED {
                    self.connected = false;
                }
                None
            },
            _ => None,
        }
    }
    
    // Remove the connection channel
    async fn remove_connection_channel(&mut self) -> Result<(), io::Error> {
        // Prepare the command packet
        let mut cmd = Vec::with_capacity(5);
        cmd.push(CMD_REMOVE_CONNECTION_CHANNEL);
        
        // Connection ID (4 bytes, little endian)
        cmd.extend_from_slice(&self.conn_id.to_le_bytes());
        
        // Send the command
        self.stream.write_all(&cmd).await?;
        
        Ok(())
    }
}

#[async_trait]
impl InputHandler for FlicButton {
    async fn listen(&mut self) -> TransitResult<InputEvent> {
        // Ensure we have a connection channel
        if !self.connected {
            self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
        }
        
        // Buffer to read protocol data
        let mut buf = [0u8; 64];
        
        loop {
            // Read from TCP stream
            let bytes_read = self.stream.read(&mut buf).await.map_err(|e| TransitError::Io(e))?;
            
            if bytes_read == 0 {
                // Connection closed, try to reconnect
                self.stream = TcpStream::connect("127.0.0.1:5551").await.map_err(|e| TransitError::Io(e))?;
                self.connected = false;
                self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
                continue;
            }
            
            // Try to parse the event
            if let Some(event) = self.parse_event(&buf[..bytes_read]) {
                return Ok(event);
            }
            
            // If we couldn't parse a valid event, continue reading
        }
    }

    async fn cleanup(&mut self) -> TransitResult<()> {
        // Remove connection channel if connected
        if self.connected {
            self.remove_connection_channel().await.map_err(|e| TransitError::Io(e))?;
        }
        
        // Close connection to flicd
        self.stream.shutdown().await.map_err(|e| TransitError::Io(e))?;
        Ok(())
    }
}
