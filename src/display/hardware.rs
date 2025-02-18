use log::debug;
use rpi_led_matrix::{LedCanvas, LedMatrix};

use crate::display::StateEvent;

use super::DisplayContext;

impl DisplayContext for LedMatrix {
    type Display = LedCanvas;

    fn show_display(&mut self, display: Self::Display) -> (Self::Display, impl Iterator<Item = StateEvent>) {
        debug!("Hardware display swap starting");
        // Try to get some debug info about the canvas state
        let new_canvas = {
            debug!("About to swap canvas");
            let result = self.swap(display);
            debug!("Canvas swap completed");
            result
        };
        debug!("Hardware display swap completed, returning new canvas");
        (new_canvas, std::iter::empty())
    }

    fn target(&mut self) -> Self::Display {
        self.offscreen_canvas()
    }
}
