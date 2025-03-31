//! Flic protocol packet definitions
//! 
//! Adapted from the Flic C++ protocol definitions at:
//! https://github.com/50ButtonsEach/fliclib-linux-hci/blob/master/simpleclient/client_protocol_packets.h
//!
//! Note: 16-bit little endian length header is prepended to each packet.
//! The length of the length field itself is not included in the length.

use std::fmt;

/// Bluetooth device address (6 bytes)
pub type BdAddr = [u8; 6];

/// Error codes for connection channel creation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CreateConnectionChannelError {
    NoError = 0,
    MaxPendingConnectionsReached = 1,
}

/// Connection status values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionStatus {
    Disconnected = 0,
    Connected = 1,
    Ready = 2,
}

/// Disconnect reason values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisconnectReason {
    Unspecified = 0,
    ConnectionEstablishmentFailed = 1,
    TimedOut = 2,
    BondingKeysMismatch = 3,
}

/// Removed reason values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RemovedReason {
    RemovedByThisClient = 0,
    ForceDisconnectedByThisClient = 1,
    ForceDisconnectedByOtherClient = 2,
    ButtonIsPrivate = 3,
    VerifyTimeout = 4,
    InternetBackendError = 5,
    InvalidData = 6,
    CouldntLoadDevice = 7,
    DeletedByThisClient = 8,
    DeletedByOtherClient = 9,
    ButtonBelongsToOtherPartner = 10,
    DeletedFromButton = 11,
}

/// Click type values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClickType {
    ButtonDown = 0,
    ButtonUp = 1,
    ButtonClick = 2,
    ButtonSingleClick = 3,
    ButtonDoubleClick = 4,
    ButtonHold = 5,
}

impl fmt::Display for ClickType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClickType::ButtonDown => write!(f, "ButtonDown"),
            ClickType::ButtonUp => write!(f, "ButtonUp"),
            ClickType::ButtonClick => write!(f, "ButtonClick"),
            ClickType::ButtonSingleClick => write!(f, "ButtonSingleClick"),
            ClickType::ButtonDoubleClick => write!(f, "ButtonDoubleClick"),
            ClickType::ButtonHold => write!(f, "ButtonHold"),
        }
    }
}

/// Bluetooth address type values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BdAddrType {
    PublicBdAddrType = 0,
    RandomBdAddrType = 1,
}

/// Latency mode values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LatencyMode {
    NormalLatency = 0,
    LowLatency = 1,
    HighLatency = 2,
}

/// Scan wizard result values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ScanWizardResult {
    WizardSuccess = 0,
    WizardCancelledByUser = 1,
    WizardFailedTimeout = 2,
    WizardButtonIsPrivate = 3,
    WizardBluetoothUnavailable = 4,
    WizardInternetBackendError = 5,
    WizardInvalidData = 6,
    WizardButtonBelongsToOtherPartner = 7,
    WizardButtonAlreadyConnectedToOtherDevice = 8,
}

/// Bluetooth controller state values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BluetoothControllerState {
    Detached = 0,
    Resetting = 1,
    Attached = 2,
}

// Command opcodes
pub const CMD_GET_INFO_OPCODE: u8 = 0;
pub const CMD_CREATE_SCANNER_OPCODE: u8 = 1;
pub const CMD_REMOVE_SCANNER_OPCODE: u8 = 2;
pub const CMD_CREATE_CONNECTION_CHANNEL_OPCODE: u8 = 3;
pub const CMD_REMOVE_CONNECTION_CHANNEL_OPCODE: u8 = 4;
pub const CMD_FORCE_DISCONNECT_OPCODE: u8 = 5;
pub const CMD_CHANGE_MODE_PARAMETERS_OPCODE: u8 = 6;
pub const CMD_PING_OPCODE: u8 = 7;
pub const CMD_GET_BUTTON_INFO_OPCODE: u8 = 8;
pub const CMD_CREATE_SCAN_WIZARD_OPCODE: u8 = 9;
pub const CMD_CANCEL_SCAN_WIZARD_OPCODE: u8 = 10;
pub const CMD_DELETE_BUTTON_OPCODE: u8 = 11;
pub const CMD_CREATE_BATTERY_STATUS_LISTENER_OPCODE: u8 = 12;
pub const CMD_REMOVE_BATTERY_STATUS_LISTENER_OPCODE: u8 = 13;

// Event opcodes
pub const EVT_ADVERTISEMENT_PACKET_OPCODE: u8 = 0;
pub const EVT_CREATE_CONNECTION_CHANNEL_RESPONSE_OPCODE: u8 = 1;
pub const EVT_CONNECTION_STATUS_CHANGED_OPCODE: u8 = 2;
pub const EVT_CONNECTION_CHANNEL_REMOVED_OPCODE: u8 = 3;
pub const EVT_BUTTON_UP_OR_DOWN_OPCODE: u8 = 4;
pub const EVT_BUTTON_CLICK_OR_HOLD_OPCODE: u8 = 5;
pub const EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK_OPCODE: u8 = 6;
pub const EVT_BUTTON_SINGLE_OR_DOUBLE_CLICK_OR_HOLD_OPCODE: u8 = 7;
pub const EVT_NEW_VERIFIED_BUTTON_OPCODE: u8 = 8;
pub const EVT_GET_INFO_RESPONSE_OPCODE: u8 = 9;
pub const EVT_NO_SPACE_FOR_NEW_CONNECTION_OPCODE: u8 = 10;
pub const EVT_GOT_SPACE_FOR_NEW_CONNECTION_OPCODE: u8 = 11;
pub const EVT_BLUETOOTH_CONTROLLER_STATE_CHANGE_OPCODE: u8 = 12;
pub const EVT_PING_RESPONSE_OPCODE: u8 = 13;
pub const EVT_GET_BUTTON_INFO_RESPONSE_OPCODE: u8 = 14;
pub const EVT_SCAN_WIZARD_FOUND_PRIVATE_BUTTON_OPCODE: u8 = 15;
pub const EVT_SCAN_WIZARD_FOUND_PUBLIC_BUTTON_OPCODE: u8 = 16;
pub const EVT_SCAN_WIZARD_BUTTON_CONNECTED_OPCODE: u8 = 17;
pub const EVT_SCAN_WIZARD_COMPLETED_OPCODE: u8 = 18;
pub const EVT_BUTTON_DELETED_OPCODE: u8 = 19;
pub const EVT_BATTERY_STATUS_OPCODE: u8 = 20;

// Click type constants
pub const BUTTON_DOWN: u8 = 0x00;
pub const BUTTON_UP: u8 = 0x01;
pub const BUTTON_CLICK: u8 = 0x02;
pub const BUTTON_HOLD: u8 = 0x03;
pub const BUTTON_SINGLE_CLICK: u8 = 0x04;
pub const BUTTON_DOUBLE_CLICK: u8 = 0x05;

// Connection status constants
pub const DISCONNECTED: u8 = 0x00;
pub const CONNECTED: u8 = 0x01;
pub const READY: u8 = 0x02;

// Latency mode constants
pub const LATENCY_NORMAL: u8 = 0x00;
pub const LATENCY_LOW: u8 = 0x01;
pub const LATENCY_HIGH: u8 = 0x02;

// Command packet structures

/// CmdGetInfo command packet
#[derive(Debug)]
pub struct CmdGetInfo {
    pub opcode: u8,
}

impl CmdGetInfo {
    pub fn new() -> Self {
        CmdGetInfo { opcode: CMD_GET_INFO_OPCODE }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![self.opcode]
    }
}

/// CmdCreateConnectionChannel command packet
#[derive(Debug)]
pub struct CmdCreateConnectionChannel {
    pub opcode: u8,
    pub conn_id: u32,
    pub bd_addr: BdAddr,
    pub latency_mode: LatencyMode,
    pub auto_disconnect_time: i16,
}

impl CmdCreateConnectionChannel {
    pub fn new(conn_id: u32, bd_addr: BdAddr, latency_mode: LatencyMode, auto_disconnect_time: i16) -> Self {
        CmdCreateConnectionChannel {
            opcode: CMD_CREATE_CONNECTION_CHANNEL_OPCODE,
            conn_id,
            bd_addr,
            latency_mode,
            auto_disconnect_time,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(13);
        bytes.push(self.opcode);
        bytes.extend_from_slice(&self.conn_id.to_le_bytes());
        bytes.extend_from_slice(&self.bd_addr);
        bytes.push(self.latency_mode as u8);
        bytes.extend_from_slice(&self.auto_disconnect_time.to_le_bytes());
        bytes
    }
}

/// CmdRemoveConnectionChannel command packet
#[derive(Debug)]
pub struct CmdRemoveConnectionChannel {
    pub opcode: u8,
    pub conn_id: u32,
}

impl CmdRemoveConnectionChannel {
    pub fn new(conn_id: u32) -> Self {
        CmdRemoveConnectionChannel {
            opcode: CMD_REMOVE_CONNECTION_CHANNEL_OPCODE,
            conn_id,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(5);
        bytes.push(self.opcode);
        bytes.extend_from_slice(&self.conn_id.to_le_bytes());
        bytes
    }
}

/// CmdForceDisconnect command packet
#[derive(Debug)]
pub struct CmdForceDisconnect {
    pub opcode: u8,
    pub bd_addr: BdAddr,
}

impl CmdForceDisconnect {
    pub fn new(bd_addr: BdAddr) -> Self {
        CmdForceDisconnect {
            opcode: CMD_FORCE_DISCONNECT_OPCODE,
            bd_addr,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(7);
        bytes.push(self.opcode);
        bytes.extend_from_slice(&self.bd_addr);
        bytes
    }
}

/// CmdPing command packet
#[derive(Debug)]
pub struct CmdPing {
    pub opcode: u8,
    pub ping_id: u32,
}

impl CmdPing {
    pub fn new(ping_id: u32) -> Self {
        CmdPing {
            opcode: CMD_PING_OPCODE,
            ping_id,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(5);
        bytes.push(self.opcode);
        bytes.extend_from_slice(&self.ping_id.to_le_bytes());
        bytes
    }
}

// Event packet structures

/// ConnectionEventBase (common base for connection-related events)
#[derive(Debug)]
pub struct ConnectionEventBase {
    pub opcode: u8,
    pub conn_id: u32,
}

impl ConnectionEventBase {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 5 {
            return None;
        }
        
        let opcode = bytes[0];
        let conn_id = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        
        Some(ConnectionEventBase {
            opcode,
            conn_id,
        })
    }
}

/// EvtCreateConnectionChannelResponse event packet
#[derive(Debug)]
pub struct EvtCreateConnectionChannelResponse {
    pub base: ConnectionEventBase,
    pub error: CreateConnectionChannelError,
    pub connection_status: ConnectionStatus,
}

impl EvtCreateConnectionChannelResponse {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 7 {
            return None;
        }
        
        let base = ConnectionEventBase::from_bytes(bytes)?;
        
        let error = match bytes[5] {
            0 => CreateConnectionChannelError::NoError,
            1 => CreateConnectionChannelError::MaxPendingConnectionsReached,
            _ => return None,
        };
        
        let connection_status = match bytes[6] {
            0 => ConnectionStatus::Disconnected,
            1 => ConnectionStatus::Connected,
            2 => ConnectionStatus::Ready,
            _ => return None,
        };
        
        Some(EvtCreateConnectionChannelResponse {
            base,
            error,
            connection_status,
        })
    }
}

/// EvtConnectionStatusChanged event packet
#[derive(Debug)]
pub struct EvtConnectionStatusChanged {
    pub base: ConnectionEventBase,
    pub connection_status: ConnectionStatus,
    pub disconnect_reason: DisconnectReason,
}

impl EvtConnectionStatusChanged {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 7 {
            return None;
        }
        
        let base = ConnectionEventBase::from_bytes(bytes)?;
        
        let connection_status = match bytes[5] {
            0 => ConnectionStatus::Disconnected,
            1 => ConnectionStatus::Connected,
            2 => ConnectionStatus::Ready,
            _ => return None,
        };
        
        let disconnect_reason = match bytes[6] {
            0 => DisconnectReason::Unspecified,
            1 => DisconnectReason::ConnectionEstablishmentFailed,
            2 => DisconnectReason::TimedOut,
            3 => DisconnectReason::BondingKeysMismatch,
            _ => return None,
        };
        
        Some(EvtConnectionStatusChanged {
            base,
            connection_status,
            disconnect_reason,
        })
    }
}

/// EvtButtonEvent event packet
#[derive(Debug)]
pub struct EvtButtonEvent {
    pub base: ConnectionEventBase,
    pub click_type: ClickType,
    pub was_queued: bool,
    pub time_diff: u32,
}

impl EvtButtonEvent {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 11 {
            return None;
        }
        
        let base = ConnectionEventBase::from_bytes(bytes)?;
        
        let click_type = match bytes[5] {
            0 => ClickType::ButtonDown,
            1 => ClickType::ButtonUp,
            _ => return None,
        };
        
        let was_queued = bytes[6] != 0;
        let time_diff = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]);
        
        Some(EvtButtonEvent {
            base,
            click_type,
            was_queued,
            time_diff,
        })
    }
}

/// EvtGetInfoResponse event packet
#[derive(Debug)]
pub struct EvtGetInfoResponse {
    pub opcode: u8,
    pub bluetooth_controller_state: BluetoothControllerState,
    pub my_bd_addr: BdAddr,
    pub my_bd_addr_type: BdAddrType,
    pub max_pending_connections: u8,
    pub max_concurrently_connected_buttons: u16,
    pub current_pending_connections: u8,
    pub currently_no_space_for_new_connection: bool,
}

impl EvtGetInfoResponse {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 14 {
            return None;
        }
        
        let opcode = bytes[0];
        if opcode != EVT_GET_INFO_RESPONSE_OPCODE {
            return None;
        }
        
        let bluetooth_controller_state = match bytes[1] {
            0 => BluetoothControllerState::Detached,
            1 => BluetoothControllerState::Resetting,
            2 => BluetoothControllerState::Attached,
            _ => return None,
        };
        
        let mut my_bd_addr = [0u8; 6];
        my_bd_addr.copy_from_slice(&bytes[2..8]);
        
        let my_bd_addr_type = match bytes[8] {
            0 => BdAddrType::PublicBdAddrType,
            1 => BdAddrType::RandomBdAddrType,
            _ => return None,
        };
        
        let max_pending_connections = bytes[9];
        let max_concurrently_connected_buttons = u16::from_le_bytes([bytes[10], bytes[11]]);
        let current_pending_connections = bytes[12];
        let currently_no_space_for_new_connection = bytes[13] != 0;
        
        Some(EvtGetInfoResponse {
            opcode,
            bluetooth_controller_state,
            my_bd_addr,
            my_bd_addr_type,
            max_pending_connections,
            max_concurrently_connected_buttons,
            current_pending_connections,
            currently_no_space_for_new_connection,
        })
    }
}

/// Adds the length prefix to a command packet
pub fn add_length_prefix(bytes: &[u8]) -> Vec<u8> {
    let len = bytes.len() as u16;
    let mut result = Vec::with_capacity(bytes.len() + 2);
    
    // Add the length prefix (little-endian)
    result.push((len & 0xff) as u8);
    result.push((len >> 8) as u8);
    
    // Add the payload
    result.extend_from_slice(bytes);
    
    result
}