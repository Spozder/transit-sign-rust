use std::error::Error;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use async_trait::async_trait;
use crate::display::StateEvent;

#[cfg(target_os = "linux")]
pub mod flic;

// Common event type for all input methods
#[derive(Debug, Clone)]
pub enum InputEvent {
    SinglePress,
    DoublePress,
    LongPress,
}

#[async_trait]
pub trait InputHandler {
    async fn listen(&mut self) -> Result<InputEvent, Box<dyn Error + Send>>;
    async fn cleanup(&mut self) -> Result<(), Box<dyn Error + Send>>;
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
    async fn listen(&mut self) -> Result<InputEvent, Box<dyn Error + Send>> {
        let mut buf = [0u8; 1];
        self.stdin.read_exact(&mut buf).await.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Handle key press, ignore other keys
        match buf[0] {
            b's' => Ok(InputEvent::SinglePress), // 's' for single press
            b'd' => Ok(InputEvent::DoublePress), // 'd' for double press
            b'l' => Ok(InputEvent::LongPress),   // 'l' for long press
            _ => Box::pin(self.listen()).await,  // Ignore other keys and use Box::pin for recursion
        }
    }

    async fn cleanup(&mut self) -> Result<(), Box<dyn Error + Send>> {
        Ok(()) // Nothing to clean up for keyboard
    }
}