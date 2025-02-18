use rpi_led_matrix::{LedCanvas, LedMatrix};

use crate::display::StateEvent;

use super::DisplayContext;

impl DisplayContext for LedMatrix {
    type Display = LedCanvas;

    fn swap<'a>(&'a mut self, display: &'a Self::Display) -> impl Iterator<Item = StateEvent> + 'a {
        debug!("Hardware display swap starting");
        // Use swap_active to avoid potential recursion
        self.swap_active(display);
        debug!("Hardware display swap completed");
        std::iter::empty()
    }
}
