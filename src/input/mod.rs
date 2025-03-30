use std::error::Error;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use async_trait::async_trait;
use crate::display::StateEvent;
use crate::error::{TransitError, TransitResult};

pub mod flic;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    Keyboard,
    Flic,
}

// Common event type for all input methods
#[derive(Debug, Clone)]
pub enum InputEvent {
    SinglePress,
    DoublePress,
    LongPress,
}

#[async_trait]
pub trait InputHandler {
    async fn listen(&mut self) -> TransitResult<InputEvent>;
    async fn cleanup(&mut self) -> TransitResult<()>;
}

pub struct KeyboardInput {
    stdin: tokio::io::Stdin,
}

impl KeyboardInput {
    pub fn new() -> Self {
        Self {
            stdin: tokio::io::stdin()
        }
    }
}

#[async_trait]
impl InputHandler for KeyboardInput {
    async fn listen(&mut self) -> TransitResult<InputEvent> {
        let mut buf = [0u8; 1];
        self.stdin.read_exact(&mut buf).await.map_err(|e| TransitError::Io(e))?;

        // Handle key press, ignore other keys
        match buf[0] {
            b's' => Ok(InputEvent::SinglePress), // 's' for single press
            b'd' => Ok(InputEvent::DoublePress), // 'd' for double press
            b'l' => Ok(InputEvent::LongPress),   // 'l' for long press
            _ => Box::pin(self.listen()).await,  // Ignore other keys and use Box::pin for recursion
        }
    }

    async fn cleanup(&mut self) -> TransitResult<()> {
        Ok(()) // Nothing to clean up for keyboard
    }
}