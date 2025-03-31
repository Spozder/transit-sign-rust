use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io;
use std::error::Error;
use async_trait::async_trait;
use std::time::Duration;
use std::fmt::Write as FmtWrite;
use std::time::Instant;
use log::debug;

use super::{InputEvent, InputHandler};
use crate::error::{TransitError, TransitResult};

// Import the flic_protocol module
use super::flic_protocol::*;

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
        debug!("Creating new FlicButton with address {}", button_addr_str);
        
        // Parse the button address (format: "xx:xx:xx:xx:xx:xx")
        let button_addr = Self::parse_bd_addr(button_addr_str)?;
        
        // Connect to the Flic daemon
        let mut stream = TcpStream::connect("127.0.0.1:5551").await?;
        
        // Set TCP_NODELAY to ensure packets are sent immediately
        // This helps prevent buffering that can lead to connection issues
        if let Err(e) = stream.set_nodelay(true) {
            debug!("Warning - Failed to set TCP_NODELAY: {}", e);
        }
        
        debug!("Connected to Flic daemon");
        
        // Use a simple connection ID
        let conn_id = 1;
        
        let mut button = Self { 
            stream,
            conn_id,
            button_addr,
            connected: false,
        };
        
        // Test the connection with a simple command
        match button.test_connection().await {
            Ok(_) => {
                // Now try to create a connection channel
                if let Err(e) = button.create_connection_channel().await {
                    debug!("Failed to create connection channel: {}", e);
                    return Err(e);
                }
                debug!("Connection channel created successfully");
            },
            Err(e) => {
                debug!("GetInfo command failed: {}", e);
                return Err(e);
            }
        }
        
        Ok(button)
    }

    // Generic method to send a command and read the response, matching C++ implementation
    async fn send_command(&mut self, cmd: &[u8], expected_resp_opcode: Option<u8>) -> Result<Vec<u8>, io::Error> {
        // Use the helper to add length prefix
        let packet = add_length_prefix(cmd);
        
        // Send the command and flush the stream to ensure it's sent immediately
        // write_all ensures all bytes are written, similar to the C++ loop
        self.stream.write_all(&packet).await?;
        self.stream.flush().await?;
        
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
                    
                    // Now read the actual payload
                    let mut response = vec![0u8; length as usize];
                    match tokio::time::timeout(Duration::from_secs(5), self.stream.read_exact(&mut response)).await {
                        Ok(Ok(_)) => {
                            // Check if it's the expected response type
                            if !response.is_empty() && response[0] == expected_opcode {
                                Ok(response)
                            } else {
                                Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!("Unexpected response opcode: 0x{:02x}, expected: 0x{:02x}", 
                                            if !response.is_empty() { response[0] } else { 0 }, 
                                            expected_opcode)
                                ))
                            }
                        },
                        Ok(Err(e)) => {
                            Err(e)
                        },
                        Err(_) => {
                            Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "Timeout reading response payload from Flic daemon"
                            ))
                        }
                    }
                },
                Ok(Err(e)) => {
                    Err(e)
                },
                Err(_) => {
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
        // Store in reverse order (little-endian)
        for (i, part) in parts.iter().enumerate() {
            match u8::from_str_radix(part, 16) {
                Ok(val) => addr[5 - i] = val,  // Reverse the byte order
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid hex value in Bluetooth address: {}", part)
                    ));
                }
            }
        }
        
        Ok(addr)
    }
    
    // Test the connection with a simple GetInfo command
    async fn test_connection(&mut self) -> Result<(), io::Error> {
        // Use the protocol structure to create the GetInfo command
        let cmd = CmdGetInfo::new();
        let cmd_bytes = cmd.to_bytes();
        
        // Send the command with length prefix
        let packet = add_length_prefix(&cmd_bytes);
        
        self.stream.write_all(&packet).await?;
        self.stream.flush().await?;
        
        // Try to read anything that comes back (with a timeout)
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Read whatever comes back for diagnostic purposes
        let mut read_buf = [0u8; 1024];
        match tokio::time::timeout(Duration::from_secs(2), self.stream.read(&mut read_buf)).await {
            Ok(Ok(n)) if n > 0 => {
                // Don't set self.connected here - it should only be set when a connection channel is established
                Ok(())
            },
            Ok(Ok(_)) => {
                // Don't set self.connected here
                Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Connection closed by peer"))
            },
            Ok(Err(e)) => {
                // Don't set self.connected here
                Err(e)
            },
            Err(_) => {
                // Even if we time out, we'll consider the daemon alive if we could send data
                // Don't set self.connected here - it should only be set when a connection channel is established
                Ok(())
            }
        }
    }
    
    // Create a connection channel to the button
    async fn create_connection_channel(&mut self) -> Result<(), io::Error> {
        debug!("Creating connection channel for button");
        
        // Use the protocol structure to create the command
        let cmd = CmdCreateConnectionChannel::new(
            self.conn_id,
            self.button_addr,
            LatencyMode::NormalLatency,
            511 // Never disconnect
        );
        
        let cmd_bytes = cmd.to_bytes();
        
        // Send the command and expect a response
        match self.send_command(&cmd_bytes, Some(EVT_CREATE_CONNECTION_CHANNEL_RESPONSE_OPCODE)).await {
            Ok(response) => {
                // Parse the response using our protocol structure
                if let Some(resp) = EvtCreateConnectionChannelResponse::from_bytes(&response) {
                    match resp.error {
                        CreateConnectionChannelError::NoError => {
                            // Connection channel created successfully
                            // Note: We do NOT set self.connected = true here because the connection
                            // starts in Disconnected state. The listen method will handle the 
                            // connection status change events and set connected = true when ready.
                            debug!("Connection channel created successfully (initial status: {:?})", resp.connection_status);
                            Ok(())
                        },
                        error => {
                            debug!("Failed to create connection channel with error: {:?}", error);
                            Err(io::Error::new(
                                io::ErrorKind::Other,
                                format!("Failed to create connection channel, error: {:?}", error)
                            ))
                        }
                    }
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Failed to parse connection channel response packet"
                    ))
                }
            },
            Err(e) => {
                Err(e)
            }
        }
    }
    
    // Remove the connection channel
    async fn remove_connection_channel(&mut self) -> Result<(), io::Error> {
        debug!("Removing connection channel {}", self.conn_id);
        
        // Use the protocol structure to create the command
        let cmd = CmdRemoveConnectionChannel::new(self.conn_id);
        let cmd_bytes = cmd.to_bytes();
        
        // No response is expected for remove_connection_channel
        self.send_command(&cmd_bytes, None).await?;
        
        debug!("Connection channel removed");
        self.connected = false;
        Ok(())
    }
    
    // Helper to parse a Flic event packet
    fn parse_event(&mut self, buf: &[u8]) -> Option<InputEvent> {
        // Check for minimum viable packet size
        if buf.len() < 5 {
            return None;
        }
        
        let opcode = buf[0];
        
        match opcode {
            EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK_OR_HOLD_OPCODE => {
                // Use our protocol structure to parse the button event
                if let Some(evt) = EvtButtonEvent::from_bytes(buf) {
                    // Verify it's for our connection
                    if evt.base.conn_id != self.conn_id {
                        return None;
                    }
                    
                    match evt.click_type {
                        ClickType::ButtonSingleClick => {
                            debug!("Button single click detected");
                            Some(InputEvent::SinglePress)
                        },
                        ClickType::ButtonDoubleClick => {
                            debug!("Button double click detected");
                            Some(InputEvent::DoublePress)
                        },
                        ClickType::ButtonHold => {
                            debug!("Button hold detected");
                            Some(InputEvent::LongPress)
                        },
                        _ => None
                    }
                } else {
                    None
                }
            },
            EVT_CONNECTION_STATUS_CHANGED_OPCODE => {
                // Use our protocol structure to parse the connection status event
                if let Some(evt) = EvtConnectionStatusChanged::from_bytes(buf) {
                    // Verify it's for our connection
                    if evt.base.conn_id != self.conn_id {
                        return None;
                    }
                    
                    let conn_status_event = match evt.connection_status {
                        ConnectionStatus::Ready => None,
                        ConnectionStatus::Disconnected => None,
                        _ => None
                    };
                    
                    // Update our connection status
                    match evt.connection_status {
                        ConnectionStatus::Ready => {
                            debug!("Flic button connection is now READY");
                            self.connected = true;
                        },
                        ConnectionStatus::Disconnected => {
                            debug!("Flic button connection is now DISCONNECTED");
                            self.connected = false;
                        },
                        _ => {}
                    }
                    conn_status_event
                } else {
                    None
                }
            },
            _ => None,
        }
    }
}

#[async_trait]
impl InputHandler for FlicButton {
    async fn listen(&mut self) -> TransitResult<InputEvent> {
        debug!("Listening for Flic button events, connection state: {}", self.connected);
        
        loop {
            // First, read the 2-byte length prefix
            let mut len_bytes = [0u8; 2];
            let read_result = tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                self.stream.read_exact(&mut len_bytes)
            ).await;

            match read_result {
                Ok(Ok(_)) => {
                    // Convert the length bytes to a u16 (little-endian)
                    let length = u16::from_le_bytes(len_bytes);
                    
                    // Now read the exact payload size
                    let mut payload = vec![0u8; length as usize];
                    match self.stream.read_exact(&mut payload).await {
                        Ok(_) => {
                            // Try to parse the event
                            if let Some(event) = self.parse_event(&payload) {
                                return Ok(event);
                            }
                            // If we couldn't parse a valid event, continue reading
                        },
                        Err(e) => {
                            if e.kind() == io::ErrorKind::UnexpectedEof {
                                // Connection closed, try to reconnect
                                debug!("Connection closed, reconnecting to Flic daemon");
                                self.stream = TcpStream::connect("127.0.0.1:5551").await.map_err(|e| TransitError::Io(e))?;
                                self.connected = false;
                                self.create_connection_channel().await.map_err(|e| TransitError::Io(e))?;
                            } else {
                                return Err(TransitError::Io(e));
                            }
                        }
                    }
                },
                Ok(Err(e)) => {
                    debug!("Error reading from stream: {}", e);
                }
                Err(_) => {
                    // Timeout waiting for data is normal, just continue the loop
                }
            }
        }
    }

    async fn cleanup(&mut self) -> TransitResult<()> {
        debug!("Cleaning up FlicButton");
        
        // Remove connection channel if connected
        if self.connected {
            self.remove_connection_channel().await.map_err(|e| TransitError::Io(e))?;
        }
        
        // Close connection to flicd
        self.stream.shutdown().await.map_err(|e| TransitError::Io(e))?;
        
        debug!("FlicButton cleanup complete");
        Ok(())
    }
}
