use std::fmt;
use std::io;
use std::error::Error as StdError;
use std::convert::From;

use tokio::task::JoinError;

// Main application error type
#[derive(Debug)]
pub enum TransitError {
    // IO errors
    Io(io::Error),
    
    // Config errors
    ConfigParse(toml::de::Error),
    ConfigMissing(String),
    
    // Transit provider errors
    ApiRequestFailed(String),
    ApiResponseInvalid(String),
    ApiRateLimited(String),

    // Transit x Config errors
    TransitConfigError(String),

    // Async Errors
    AsyncError(String),
    
    // Display errors
    DisplayInitFailed(String),
    RenderFailed(String),
    
    // Input errors
    InputHandlerFailed(String),
    
    // Other errors
    Other(String),
    
    // External errors that implement Send
    External(String),
}

// Implement Display for nice error messages
impl fmt::Display for TransitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::ConfigParse(err) => write!(f, "Config parse error: {}", err),
            Self::ConfigMissing(msg) => write!(f, "Missing configuration: {}", msg),
            Self::ApiRequestFailed(msg) => write!(f, "API request failed: {}", msg),
            Self::ApiResponseInvalid(msg) => write!(f, "Invalid API response: {}", msg),
            Self::ApiRateLimited(msg) => write!(f, "API rate limited: {}", msg),
            Self::TransitConfigError(msg) => write!(f, "Transit config error: {}", msg),
            Self::AsyncError(msg) => write!(f, "Async error: {}", msg),
            Self::DisplayInitFailed(msg) => write!(f, "Display initialization failed: {}", msg),
            Self::RenderFailed(msg) => write!(f, "Render failed: {}", msg),
            Self::InputHandlerFailed(msg) => write!(f, "Input handler failed: {}", msg),
            Self::Other(msg) => write!(f, "Other error: {}", msg),
            Self::External(msg) => write!(f, "External error: {}", msg),
        }
    }
}

// Implement Error trait
impl StdError for TransitError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::ConfigParse(err) => Some(err),
            _ => None,
        }
    }
}

// Convert from Box<dyn Error + Send> to TransitError
impl From<Box<dyn StdError + Send>> for TransitError {
    fn from(err: Box<dyn StdError + Send>) -> Self {
        Self::External(err.to_string())
    }
}

// From implementations for common error conversions
impl From<io::Error> for TransitError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<toml::de::Error> for TransitError {
    fn from(err: toml::de::Error) -> Self {
        Self::ConfigParse(err)
    }
}

impl From<reqwest::Error> for TransitError {
    fn from(err: reqwest::Error) -> Self {
        Self::ApiRequestFailed(err.to_string())
    }
}

impl From<serde_json::Error> for TransitError {
    fn from(err: serde_json::Error) -> Self {
        Self::ApiResponseInvalid(err.to_string())
    }
}

impl From<chrono::ParseError> for TransitError {
    fn from(err: chrono::ParseError) -> Self {
        Self::ApiResponseInvalid(err.to_string())
    }
}

impl From<JoinError> for TransitError {
    fn from(err: JoinError) -> Self {
        Self::AsyncError(err.to_string())
    }
}

// Add a context method to add more information to errors
pub trait ErrorExt<T> {
    fn context(self, message: impl Into<String>) -> TransitResult<T>;
}

impl<T, E: StdError + 'static> ErrorExt<T> for Result<T, E> {
    fn context(self, message: impl Into<String>) -> TransitResult<T> {
        self.map_err(|e| TransitError::Other(format!("{}: {}", message.into(), e)))
    }
}

// Define a Result type alias for convenience
pub type TransitResult<T> = Result<T, TransitError>;