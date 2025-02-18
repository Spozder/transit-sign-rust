use std::time::Instant;
use serde::{Deserialize, Serialize};
use crate::transit::{TransitIdentifier, TransitState};

use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb888;

#[cfg(target_os = "macos")]
use embedded_graphics_simulator::Window;

#[cfg(target_os = "linux")]
use rpi_led_matrix::{
    LedMatrix,
    LedMatrixOptions,
    LedRuntimeOptions
};

// Events that can trigger state transitions
#[derive(Debug, Clone)]
pub enum StateEvent {
    NextPage,
    NextSubpage,
    Reset,
    Quit,
    TransitUpdate,
    CustomMessage(String),
    DisplayRefresh,
}

#[derive(Debug, Clone)]
pub enum DisplayMode {
    // Normal mode showing transit state
    Transit {
        transit_identifier: TransitIdentifier,
        transit_state: Option<TransitState>, // Optional because we might not have data yet
        last_update: Instant,
    },
    // Showing a custom message
    CustomMessage {
        message: String,
        start_time: Instant,
        previous_state: Box<DisplayMode>,
    },
    // Error state
    Error {
        message: String,
        start_time: Instant,
    }
}

#[derive(Debug, Clone)]
pub enum PageDisplayHandler {
    PredictionsDisplay,
    BikeInventoryDisplay
}

impl PageDisplayHandler {
    pub fn from_key(key: &str) -> Self {
        match key {
            "predictions" => Self::PredictionsDisplay,
            "bike_inventory" => Self::BikeInventoryDisplay,
            _ => panic!("Invalid page display handler key: {}", key),
        }
    }
}

pub trait DisplayContext {
    type Display: DrawTarget<Color = Rgb888>;

    fn swap<'a>(&'a mut self, display: &'a Self::Display) -> impl Iterator<Item = StateEvent> + 'a;
}

// A wrapper type that holds both the display context and its drawable target
pub struct Display<C: DisplayContext> {
    context: C,
    target: C::Display,
    pub y_offset: i32,
}

impl<C: DisplayContext> Display<C> {
    pub fn new(context: C, target: C::Display, y_offset: i32) -> Self {
        Self { context, target, y_offset }
    }

    pub fn context_mut(&mut self) -> &mut C {
        &mut self.context
    }

    pub fn target_mut(&mut self) -> &mut C::Display {
        &mut self.target
    }

    // Add a method that handles the swap internally
    pub fn swap_display<'a>(&'a mut self) -> impl Iterator<Item = StateEvent> + 'a {
        self.context.swap(&self.target)
    }
}

// macOS Implementation - Simulator
#[cfg(target_os = "macos")]
mod sim;

// Factory function to create the appropriate display implementation
#[cfg(target_os = "macos")]
pub fn get_display() -> Display<Window> {
    let context = sim::setup_container();
    let target = sim::setup_drawable();
    Display::new(context, target, 7)
}

#[cfg(target_os = "linux")]
mod hardware;

#[cfg(target_os = "linux")]
pub fn get_display() -> Display<LedMatrix> {
    let mut options = LedMatrixOptions::new();
    options.set_rows(16);
    options.set_cols(96);
    options.set_brightness(35);
    options.set_chain_length(3);
    options.set_hardware_mapping("adafruit-hat");

    let mut rt_options = LedRuntimeOptions::new();
    rt_options.set_gpio_slowdown(3);
    let matrix = LedMatrix::new(Some(options), Some(rt_options)).unwrap();
    let mut canvas = matrix.canvas();

    Display::new(matrix, canvas, 7)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "YELLOW" => Some(Self { red: 255, green: 255, blue: 51 }),
            "RED" => Some(Self { red: 255, green: 0, blue: 0 }),
            "GREEN" => Some(Self { red: 51, green: 153, blue: 51 }),
            "BLUE" => Some(Self { red: 0, green: 153, blue: 204 }),
            "PURPLE" => Some(Self { red: 163, green: 24, blue: 163 }),
            _ => None
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self { red: 255, green: 255, blue: 255 }
    }
}

pub mod fsm;
