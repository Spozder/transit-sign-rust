use embedded_graphics_simulator::{
    SimulatorDisplay,
    Window,
    OutputSettingsBuilder,
    SimulatorEvent,
    sdl2::Keycode
};
use embedded_graphics_core::{
    pixelcolor::Rgb888,
    prelude::*,
};

use crate::display::StateEvent;

use super::DisplayContext;

impl DisplayContext for Window {
    type Display = SimulatorDisplay<Rgb888>;

    fn show_display<'a>(&'a mut self, display: &'a Self::Display) -> impl Iterator<Item = StateEvent> + 'a {
        self.update(display);
        self.events().filter_map(|event| {
            match event {
                SimulatorEvent::Quit => Some(StateEvent::Quit),
                SimulatorEvent::KeyDown { keycode, keymod: _, repeat: _ } => {
                    match keycode {
                        Keycode::S => Some(StateEvent::NextPage),
                        Keycode::D => Some(StateEvent::NextSubpage),
                        Keycode::R => Some(StateEvent::Reset),
                        Keycode::Q => Some(StateEvent::Quit),
                        _ => None,
                    }
                },
                _ => None,
            }
        })
    }
}

pub fn setup_container() -> Window {
    let output_settings = OutputSettingsBuilder::new()
        .scale(8)
        .pixel_spacing(1)
        .build();
    Window::new("Transit Sign", &output_settings)
}

pub fn setup_drawable() -> SimulatorDisplay<Rgb888> {
    SimulatorDisplay::new(Size::new(96, 16))
}