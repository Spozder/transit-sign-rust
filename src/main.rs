use std::env;
use std::error::Error;
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;
use embedded_graphics::{
    prelude::*,
    pixelcolor::Rgb888,
    draw_target::DrawTarget,
    primitives::{Circle, PrimitiveStyle},
};

mod display;
mod transit;
mod config;
mod input;

use display::{Display, DisplayContext, DisplayMode, StateEvent};
use display::fsm::DisplayFiniteStateMachine;

use transit::state::TransitStateManager;

use input::{InputEvent, InputHandler, KeyboardInput};

pub type SharedTransitStateManager = Arc<RwLock<TransitStateManager>>;
pub type SharedDisplayFiniteStateMachine = Arc<RwLock<DisplayFiniteStateMachine>>;

async fn create_input_handler() -> Result<Box<dyn InputHandler + Send>, Box<dyn Error>> {
    Ok(Box::new(KeyboardInput::new()))
}

fn console_display(display_mode: &DisplayMode, page_idx: usize, subpage_idx: usize) {
    // Clear screen (ANSI escape code)
    print!("\x1B[2J\x1B[1;1H");
            
    // Display current time
    println!("Current Time: {}", chrono::Local::now().format("%I:%M:%S %p"));

    // Display current page and subpage
    println!("Page: {}, Subpage: {}", page_idx + 1, subpage_idx + 1);
    println!("-----------------------------------------");

    match display_mode {
        DisplayMode::Transit { transit_identifier, transit_state, last_update } => {
            if let Some(transit_state) = transit_state {
                transit_state.console_display(transit_identifier.clone());
            }
        },
        DisplayMode::CustomMessage { message, start_time, previous_state } => {
            println!("{}", message);
        },
        DisplayMode::Error { message, start_time } => {
            println!("Error: {}", message);
        },
    }
}

fn graphics_display<'a, C>(
    display: &'a mut Display<C>,
    display_mode: &DisplayMode,
    page_idx: usize,
    subpage_idx: usize
) -> impl Iterator<Item = StateEvent> + 'a where 
    C: DisplayContext,
    <C::Display as DrawTarget>::Error: std::fmt::Debug
{
    match display_mode {
        DisplayMode::Transit { transit_identifier, transit_state, last_update } => {
            if let Some(transit_state) = transit_state {
                transit_state.draw(display, page_idx, subpage_idx);
            }
        },
        _ => (),
    }
    
    // Swap the display using our safe method
    display.swap_display()
}

async fn transit_update_task(shared_transit_manager: SharedTransitStateManager, display_fsm: SharedDisplayFiniteStateMachine) {
    loop {
        // Make updates in smaller scopes to ensure the locks are dropped before the next step
        {
            let mut transit_manager = shared_transit_manager.write().await;
            transit_manager.update_state().await;
        }

        {
            let mut display_fsm = display_fsm.write().await;
            display_fsm.handle_event(StateEvent::TransitUpdate).await;
        }

        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

fn run_display_loop(display_fsm: SharedDisplayFiniteStateMachine) {
    let mut display = display::get_display();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let display_mode = env::var("DISPLAY_MODE").unwrap_or(String::from("console"));

    println!("Running in {} mode", display_mode);

    if display_mode == "console" {
        loop {
            // Run the async FSM update in a blocking context
            rt.block_on(async {
                let mut display_fsm_write = display_fsm.write().await;
                display_fsm_write.handle_event(StateEvent::DisplayRefresh).await;
            });

            // Get the current state
            let current_state: (DisplayMode, usize, usize) = rt.block_on(async {
                let display_fsm_read = display_fsm.read().await;
                (display_fsm_read.current_state().clone(), display_fsm_read.page_idx, display_fsm_read.subpage_idx)
            });

            console_display(&current_state.0, current_state.1, current_state.2);
            std::thread::sleep(Duration::from_millis(1000)); // Slower refresh for console mode
        }
    } else {
        // Graphics mode
        'running: loop {
            // Run the async FSM update in a blocking context
            rt.block_on(async {
                let mut display_fsm_write = display_fsm.write().await;
                display_fsm_write.handle_event(StateEvent::DisplayRefresh).await;
            });

            // Get the current state
            let current_state: (DisplayMode, usize, usize) = rt.block_on(async {
                let display_fsm_read = display_fsm.read().await;
                (display_fsm_read.current_state().clone(), display_fsm_read.page_idx, display_fsm_read.subpage_idx)
            });

            // Update display
            let events = graphics_display(&mut display, &current_state.0, current_state.1, current_state.2);

            for event in events {
                match event {
                    StateEvent::Quit => {
                        println!("Quitting");
                        break 'running;
                    }
                    _ => {
                        rt.block_on(async {
                            let mut display_fsm_write = display_fsm.write().await;
                            display_fsm_write.handle_event(event).await;
                        })
                    }
                }
            };

            std::thread::sleep(Duration::from_millis(16)); // ~60 FPS for simulator
        }
    }
}

async fn input_handler_task(display_fsm: SharedDisplayFiniteStateMachine) {
    let mut input_handler = create_input_handler().await.expect("Failed to create input handler");
    loop {
        let event = input_handler.listen().await.expect("Failed to listen for input");
        {
            let mut display_fsm = display_fsm.write().await;
            match event {
                InputEvent::SinglePress => {
                    println!("Single press");
                    display_fsm.handle_event(StateEvent::NextPage).await;
                }
                InputEvent::DoublePress => {
                    println!("Double press");
                    display_fsm.handle_event(StateEvent::NextSubpage).await;
                }
                InputEvent::LongPress => {
                    println!("Long press");
                    display_fsm.handle_event(StateEvent::Reset).await;
                }
            }
        }   
    }
}
    

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();
    
    // Load environment variables from .env file
    match dotenv::dotenv() {
        Ok(_) => println!("Successfully loaded .env file"),
        Err(e) => println!("Failed to load .env file: {}", e),
    };
    
    // Get API keys from environment variables
    let bart_api_key = env::var("BART_API_KEY").expect("BART_API_KEY must be set");
    let muni_api_key = env::var("MUNI_API_KEY").expect("MUNI_API_KEY must be set");

    let config = config::Config::load()?;
    let display_config = config::DisplayConfig::load()?;
    
    // Initialize transit state manager
    let transit_manager = TransitStateManager::new(config, bart_api_key, muni_api_key);
    let shared_transit_manager = Arc::new(RwLock::new(transit_manager));

    let display_fsm = DisplayFiniteStateMachine::new(display_config, shared_transit_manager.clone());
    let shared_display_fsm = Arc::new(RwLock::new(display_fsm));

    println!("Transit Sign Starting...");
    println!("Press Ctrl+C to exit");

    // Create the runtime for async tasks
    let rt = tokio::runtime::Runtime::new()?;

    // Spawn background tasks
    rt.spawn(transit_update_task(shared_transit_manager.clone(), shared_display_fsm.clone()));
    rt.spawn(input_handler_task(shared_display_fsm.clone()));

    // Run the display loop in the main thread
    run_display_loop(shared_display_fsm);

    // Drop the runtime
    rt.shutdown_background();

    Ok(())
}
