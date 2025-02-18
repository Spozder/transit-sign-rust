use rpi_led_matrix::{LedCanvas, LedMatrix};

use crate::display::StateEvent;

use super::DisplayContext;

impl DisplayContext for LedMatrix {
    type Display = LedCanvas;

    fn swap<'a>(&'a mut self, display: &'a Self::Display) -> impl Iterator<Item = StateEvent> + 'a {
        self.swap(display);
        std::iter::empty()
    }
}
