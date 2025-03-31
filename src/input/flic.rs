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

    // Generic method to send a command and read the response, matching C++ implementation
    async fn send_command(&mut self, cmd: &[u8], expected_resp_opcode: Option<u8>) -> Result<Vec<u8>, io::Error> {
        // Use the helper to add length prefix
        let packet = add_length_prefix(cmd);
        
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
    
    // Test the connection with a simple GetInfo command
    async fn test_connection(&mut self) -> Result<(), io::Error> {
        println!("test_connection: Sending GetInfo command");
        
        // Use the protocol structure to create the GetInfo command
        let cmd = CmdGetInfo::new();
        let cmd_bytes = cmd.to_bytes();
        println!("test_connection: Command bytes: {}", format_bytes(&cmd_bytes));
        
        // Send the command with length prefix
        let packet = add_length_prefix(&cmd_bytes);
        println!("test_connection: Full packet with length prefix: {}", format_bytes(&packet));
        
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
    
    // Create a connection channel to the button
    async fn create_connection_channel(&mut self) -> Result<(), io::Error> {
        println!("create_connection_channel: Creating connection channel for button {:?}", format_bytes(&self.button_addr));
        
        // Use the protocol structure to create the command
        let cmd = CmdCreateConnectionChannel::new(
            self.conn_id,
            self.button_addr,
            LatencyMode::NormalLatency,
            511 // Never disconnect
        );
        
        let cmd_bytes = cmd.to_bytes();
        println!("create_connection_channel: Command bytes: {}", format_bytes(&cmd_bytes));
        
        // Send the command and expect a response
        match self.send_command(&cmd_bytes, Some(EVT_CREATE_CONNECTION_CHANNEL_RESPONSE_OPCODE)).await {
            Ok(response) => {
                // Parse the response using our protocol structure
                if let Some(resp) = EvtCreateConnectionChannelResponse::from_bytes(&response) {
                    println!("create_connection_channel: Parsed response: error={:?}, status={:?}", resp.error, resp.connection_status);
                    
                    match resp.error {
                        CreateConnectionChannelError::NoError => {
                            // Connection channel created successfully
                            // Note: We do NOT set self.connected = true here because the connection
                            // starts in Disconnected state. The listen method will handle the 
                            // connection status change events and set connected = true when ready.
                            println!("create_connection_channel: Connection channel created successfully (initial status: {:?})", resp.connection_status);
                            Ok(())
                        },
                        error => {
                            println!("create_connection_channel: Failed with error: {:?}", error);
                            Err(io::Error::new(
                                io::ErrorKind::Other,
                                format!("Failed to create connection channel, error: {:?}", error)
                            ))
                        }
                    }
                } else {
                    println!("create_connection_channel: Failed to parse response packet");
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Failed to parse connection channel response packet"
                    ))
                }
            },
            Err(e) => {
                println!("create_connection_channel: Failed to create connection channel: {}", e);
                Err(e)
            }
        }
    }
    
    // Remove the connection channel
    async fn remove_connection_channel(&mut self) -> Result<(), io::Error> {
        println!("remove_connection_channel: Removing connection channel {}", self.conn_id);
        
        // Use the protocol structure to create the command
        let cmd = CmdRemoveConnectionChannel::new(self.conn_id);
        let cmd_bytes = cmd.to_bytes();
        
        println!("remove_connection_channel: Command bytes: {}", format_bytes(&cmd_bytes));
        
        // No response is expected for remove_connection_channel
        self.send_command(&cmd_bytes, None).await?;
        
        println!("remove_connection_channel: Connection channel removed");
        self.connected = false;
        Ok(())
    }
    
    // Helper to parse a Flic event packet
    fn parse_event(&mut self, buf: &[u8]) -> Option<InputEvent> {
        // Check for minimum viable packet size
        if buf.len() < 5 {
            println!("parse_event: Packet too small: {} bytes", buf.len());
            return None;
        }
        
        let opcode = buf[0];
        println!("parse_event: Parsing event with opcode: 0x{:02x} ({})", opcode, opcode);
        
        match opcode {
            EVT_BUTTON_UP_OR_DOWN_OPCODE => {
                // Use our protocol structure to parse the button event
                if let Some(evt) = EvtButtonEvent::from_bytes(buf) {
                    // Verify it's for our connection
                    if evt.base.conn_id != self.conn_id {
                        println!("parse_event: Ignoring button event for different connection (ID: {})", evt.base.conn_id);
                        return None;
                    }
                    
                    println!("parse_event: Button event, click_type: {:?}, was_queued: {}", evt.click_type, evt.was_queued);
                    
                    match evt.click_type {
                        ClickType::ButtonDown => {
                            println!("parse_event: BUTTON_DOWN detected");
                            None
                        },
                        ClickType::ButtonUp => {
                            println!("parse_event: BUTTON_UP detected - ignoring");
                            None
                        },
                        _ => {
                            println!("parse_event: Unexpected click type: {:?}", evt.click_type);
                            None
                        }
                    }
                } else {
                    println!("parse_event: Failed to parse button event packet");
                    None
                }
            },
            EVT_CONNECTION_STATUS_CHANGED_OPCODE => {
                // Use our protocol structure to parse the connection status event
                if let Some(evt) = EvtConnectionStatusChanged::from_bytes(buf) {
                    // Verify it's for our connection
                    if evt.base.conn_id != self.conn_id {
                        println!("parse_event: Ignoring connection status for different connection (ID: {})", evt.base.conn_id);
                        return None;
                    }
                    
                    let conn_status_event = match evt.connection_status {
                        ConnectionStatus::Ready => {
                            println!("parse_event: Button is now READY");
                            None
                        },
                        ConnectionStatus::Disconnected => {
                            println!("parse_event: Button is now DISCONNECTED");
                            None
                        },
                        _ => None
                    };
                    
                    println!("parse_event: Connection status changed event, status: {:?}", evt.connection_status);
                    
                    // Update our connection status
                    match evt.connection_status {
                        ConnectionStatus::Ready => {
                            println!("parse_event: Connection is now READY");
                            self.connected = true;
                        },
                        ConnectionStatus::Disconnected => {
                            println!("parse_event: Connection is now DISCONNECTED");
                            self.connected = false;
                        },
                        _ => {}
                    }
                    conn_status_event
                } else {
                    println!("parse_event: Failed to parse connection status event packet");
                    None
                }
            },
            _ => {
                println!("parse_event: Unknown or unhandled opcode: 0x{:02x}", opcode);
                None
            },
        }
    }
}

#[async_trait]
impl InputHandler for FlicButton {
    async fn listen(&mut self) -> TransitResult<InputEvent> {
        // Print stream information for debugging
        match self.stream.peer_addr() {
            Ok(peer_addr) => println!("listen: Connected to peer address: {}", peer_addr),
            Err(e) => println!("listen: Could not get peer address: {}", e),
        }
        
        match self.stream.local_addr() {
            Ok(local_addr) => println!("listen: Local address: {}", local_addr),
            Err(e) => println!("listen: Could not get local address: {}", e),
        }
        
        println!("listen: Stream connection state: connected={}", self.connected);
        println!("listen: Starting to listen for button events");
        
        loop {
            println!("listen: Waiting for data from Flic daemon");
            
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
                Ok(Err(e)) => {
                    println!("listen: Error reading from stream: {}", e);
                }
                Err(_) => {
                    println!("listen: Timeout waiting for data from Flic daemon");
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
