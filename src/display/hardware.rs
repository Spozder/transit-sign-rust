use log::debug;
use rpi_led_matrix::{LedCanvas, LedMatrix};

use crate::display::StateEvent;

use super::DisplayContext;

impl DisplayContext for LedMatrix {
    type Display = LedCanvas;

    fn show_display(&mut self, display: Self::Display) -> (Self::Display, impl Iterator<Item = StateEvent>) {
        debug!("Hardware display swap starting");
        let new_canvas = self.swap(display);
        debug!("Hardware display swap completed");
        (new_canvas, std::iter::empty())
    }

    fn target(&mut self) -> Self::Display {
        self.offscreen_canvas()
    }
}
