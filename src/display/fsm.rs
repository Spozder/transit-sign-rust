use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::error::Error;

use crate::config::DisplayConfig;
use super::{Color, DisplayMode, StateEvent};
use crate::transit::{TransitIdentifier, TransitState};

use super::super::SharedTransitStateManager;

// Main state machine struct
pub struct DisplayFiniteStateMachine {
    current_state: DisplayMode,
    config: DisplayConfig,  // Stores page list, other display options
    pub page_idx: usize,
    pub subpage_idx: usize,
    
    transit_manager: SharedTransitStateManager
}

impl DisplayFiniteStateMachine {
    pub fn new(config: DisplayConfig, transit_manager: SharedTransitStateManager) -> Self {
        let initial_transit_identifier = config.pages[0].subpages[0].transit_identifier.clone();

        Self {
            current_state: DisplayMode::Transit {
                transit_identifier: initial_transit_identifier,
                transit_state: None,
                last_update: Instant::now(),
            },
            config,
            page_idx: 0,
            subpage_idx: 0,
            transit_manager
        }
    }

    pub async fn handle_event(&mut self, event: StateEvent) {
        match event {
            StateEvent::NextPage => self.handle_next_page().await,
            StateEvent::NextSubpage => self.handle_next_subpage().await,
            StateEvent::Reset => self.handle_reset().await,
            StateEvent::TransitUpdate => self.handle_transit_update().await,
            StateEvent::CustomMessage(msg) => self.handle_custom_message(msg),
            StateEvent::DisplayRefresh => self.handle_display_refresh().await,
            _ => (),
        };
    }

    pub fn current_state(&self) -> &DisplayMode {
        &self.current_state
    }

    async fn get_state_for_identifier(&self, transit_identifier: &TransitIdentifier) -> TransitState {
        self.transit_manager.read().await.get_state_for_identifier(transit_identifier)
    }

    async fn handle_next_page(&mut self) {
        match &self.current_state {
            DisplayMode::Transit { .. } => {
                let next_page_idx = (self.page_idx + 1) % self.config.pages.len();
                let subpage_idx = 0;
                let next_transit_identifier = self.config.pages[next_page_idx].subpages[subpage_idx].transit_identifier.clone();

                let state_for_identifier = self.get_state_for_identifier(&next_transit_identifier).await;
                self.current_state = DisplayMode::Transit {
                    transit_identifier: next_transit_identifier,
                    transit_state: Some(state_for_identifier),
                    last_update: Instant::now(),
                };
                self.page_idx = next_page_idx;
                self.subpage_idx = subpage_idx;
            },
            DisplayMode::CustomMessage { .. } => {
                let first_transit_identifier = self.config.pages[0].subpages[0].transit_identifier.clone();
                let state_for_identifier = self.get_state_for_identifier(&first_transit_identifier).await;
                self.current_state = DisplayMode::Transit {
                    transit_identifier: first_transit_identifier,
                    transit_state: Some(state_for_identifier),
                    last_update: Instant::now(),
                };
                self.page_idx = 0;
                self.subpage_idx = 0;
            },
            DisplayMode::Error { .. } => (),
        }
    }

    async fn handle_next_subpage(&mut self) {
        let next_subpage_idx = (self.subpage_idx + 1) % self.config.pages[self.page_idx].subpages.len();
        let next_transit_identifier = self.config.pages[self.page_idx].subpages[next_subpage_idx].transit_identifier.clone();
        let state_for_identifier = self.get_state_for_identifier(&next_transit_identifier).await;

        match &self.current_state {
            DisplayMode::Error { .. } => (),
            _ => {
                self.current_state = DisplayMode::Transit {
                    transit_identifier: next_transit_identifier,
                    transit_state: Some(state_for_identifier),
                    last_update: Instant::now(),
                };
                self.subpage_idx = next_subpage_idx;
            }
        }
    }

    async fn handle_reset(&mut self) {
        let first_transit_identifier = self.config.pages[0].subpages[0].transit_identifier.clone();
        let state_for_identifier = self.get_state_for_identifier(&first_transit_identifier).await;
        self.current_state = DisplayMode::Transit {
            transit_identifier: first_transit_identifier,
            transit_state: Some(state_for_identifier),
            last_update: Instant::now(),
        };
        self.page_idx = 0;
        self.subpage_idx = 0;
    }

    async fn handle_transit_update(&mut self) {
        match &self.current_state {
            DisplayMode::Transit { transit_identifier, .. } => {
                let new_state = self.get_state_for_identifier(&transit_identifier).await;

                self.current_state = DisplayMode::Transit {
                    transit_identifier: transit_identifier.clone(),
                    transit_state: Some(new_state),
                    last_update: Instant::now(),
                };
            }
            DisplayMode::CustomMessage { .. } | DisplayMode::Error { .. } => (),
        }
    }

    fn handle_custom_message(&mut self, message: String) {
        self.current_state = DisplayMode::CustomMessage {
            message,
            start_time: Instant::now(),
            previous_state: Box::new(self.current_state.clone()),
        };
    }

    async fn handle_display_refresh(&mut self) {
        match &self.current_state {
            DisplayMode::CustomMessage { previous_state, start_time, .. } => {
                if start_time.elapsed() >= self.config.message_timeout {
                    self.current_state = *previous_state.clone();
                }
            }
            DisplayMode::Error { start_time, .. } => {
                if start_time.elapsed() >= self.config.error_timeout {
                    self.handle_reset().await;
                }
            }
            _ => (),
        }
    }
}
