use log::debug;
use async_trait::async_trait;
use chrono::{DateTime, Utc, Local};
use serde::{Deserialize, Serialize};
use std::iter;
use itertools::Itertools;
use embedded_graphics::{
    prelude::*,
    primitives::PrimitiveStyle,
    text::Text,
    mono_font::MonoTextStyle,
    mono_font::ascii::FONT_5X7,
    pixelcolor::Rgb888,
};

use crate::config::Stop;
use crate::display::{Color, Display, DisplayContext};

#[derive(Eq, Hash, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct TransitIdentifier {
    pub provider_key: String,
    pub station_id: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub provider_key: String,
    pub route_name: String,
    pub destination: String,
    pub minutes_until_arrival: i32,
    pub predicted_time: DateTime<Utc>,
    pub station_id: String,
    pub stop_id: String,
    pub direction: String,
    pub color: Color
}

impl Prediction {
    pub fn to_identifier(&self) -> TransitIdentifier {
        TransitIdentifier {
            provider_key: self.provider_key.clone(),
            station_id: self.station_id.clone(),
            direction: self.direction.clone(),
        }
    }

    pub fn to_display_string(&self) -> String {
        match self.provider_key.as_str() {
            "bart" => format!("{} to {}: {} min", self.route_name.chars().take(1).collect::<String>(), self.destination, self.minutes_until_arrival),
            "muni" => format!("{} {}: {} min", self.route_name, self.direction, self.minutes_until_arrival),
            _ => "Unsupported".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BikeInventory {
    pub station_name: String,
    pub bikes_available: i32,
    pub docks_available: i32,
    pub ebikes_v1_available: i32,
    pub ebikes_v2_available: i32,
    pub last_updated: DateTime<Utc>,
    pub stop_id: String
}

impl BikeInventory {
    pub fn to_identifier(&self) -> TransitIdentifier {
        TransitIdentifier {
            provider_key: "baywheels".to_string(),
            station_id: self.station_name.clone(),
            direction: "None".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TransitState {
    Predictions(Vec<Prediction>), // For BART and Muni
    BikeInventory(BikeInventory), // For BayWheels,
    #[default]
    EmptyState,
}

impl TransitState {
    pub fn to_state_updates(&self) -> Box<dyn Iterator<Item = (TransitIdentifier, TransitState)> + '_> {
        match self {
            TransitState::Predictions(predictions) => {
                // First map each prediction to (identifier, prediction)
                Box::new(
                    predictions.iter()
                        .map(|pred| (pred.to_identifier(), pred.clone()))
                        .into_group_map()
                        .into_iter()
                        .map(|(identifier, predictions)| {
                            let ordered_predictions = predictions.into_iter().sorted_by(|a, b| a.minutes_until_arrival.cmp(&b.minutes_until_arrival)).collect();
                            (identifier, TransitState::Predictions(ordered_predictions))
                        })
                )
            },
            TransitState::BikeInventory(inventory) => Box::new(
                iter::once((inventory.to_identifier(), self.clone()))
            ),
            TransitState::EmptyState => Box::new(iter::empty())
        }
    }

    pub fn console_display(&self, transit_identifier: TransitIdentifier) {
        match self {
            TransitState::Predictions(predictions) => Self::console_display_predictions(transit_identifier, predictions),
            TransitState::BikeInventory(inventory) => Self::console_display_bike_inventory(transit_identifier, inventory),
            TransitState::EmptyState => println!("No data available"),
        }
    }

    fn console_display_predictions(transit_identifier: TransitIdentifier, predictions: &[Prediction]) {
        println!("\n{} - {} - {}", transit_identifier.station_id, transit_identifier.provider_key, transit_identifier.direction);
        if predictions.is_empty() {
            println!("  No upcoming departures");
            return;
        }

        for pred in predictions.iter() {
            println!("  {} to {} - {} minutes",
                     pred.route_name, pred.destination, pred.minutes_until_arrival)
        }
    }

    fn console_display_bike_inventory(transit_identifier: TransitIdentifier, inventory: &BikeInventory) {
        println!("\n{} - {} - {}", transit_identifier.station_id, transit_identifier.provider_key, transit_identifier.direction);
        println!("  Bikes: {}", inventory.bikes_available);
        println!("  Docks: {}", inventory.docks_available);
        println!("  eBikes (v1): {}", inventory.ebikes_v1_available);
        println!("  eBikes (v2): {}", inventory.ebikes_v2_available);
        println!("  Last Updated: {}", inventory.last_updated.with_timezone(&Local).format("%I:%M %p"));
    }

    pub fn draw<C : DisplayContext>(&self, display: &mut Display<C>, page_idx: usize, subpage_idx: usize)
    where
        C: DisplayContext,
        C::Display: DrawTarget<Color = Rgb888>,
        <C::Display as DrawTarget>::Error: std::fmt::Debug
{
        debug!("Drawing transit state: {:?}", self);
        match self {
            TransitState::Predictions(predictions) => {
                debug!("Drawing predictions: {} items", predictions.len());
                Self::draw_predictions(display, predictions);
            },
            TransitState::BikeInventory(inventory) => {
                debug!("Drawing bike inventory for {}", inventory.station_name);
                Self::draw_bike_inventory(display, inventory);
            },
            TransitState::EmptyState => {
                debug!("Empty state, nothing to draw");
            },
        }
    }

    fn draw_predictions<C>(
        display: &mut Display<C>,
        predictions: &[Prediction]
    ) where
        C: DisplayContext,
        C::Display: DrawTarget<Color = Rgb888>,
        <C::Display as DrawTarget>::Error: std::fmt::Debug
    {        
        // Clear display by drawing black rectangle
        embedded_graphics::primitives::Rectangle::new(
            Point::new(0, 0),
            Size::new(96, 16) // Assuming 96x16 display, adjust as needed
        )
        .into_styled(PrimitiveStyle::with_fill(Rgb888::BLACK))
        .draw(display.target_mut())
        .unwrap();
        
        // Predictions are sorted by minutes_until_arrival, take the first two
        let predictions_to_show = predictions.iter().take(2);

        // Draw each prediction
        for (i, pred) in predictions_to_show.enumerate() {
            let y_pos = (i as i32) * 8 + display.y_offset; // 8 pixels between predictions
            
            // Draw route name, destination, and arrival time
            Text::new(
                &pred.to_display_string().as_str(),
                Point::new(1, y_pos),
                MonoTextStyle::new(&FONT_5X7, Rgb888::new(pred.color.red, pred.color.green, pred.color.blue))
            )
            .draw(display.target_mut())
            .unwrap();
        }
    }

    fn draw_bike_inventory<C : DisplayContext>(
        display: &mut Display<C>,
        inventory: &BikeInventory
    ) where
        C: DisplayContext,
        C::Display: DrawTarget<Color = Rgb888>,
        <C::Display as DrawTarget>::Error: std::fmt::Debug
    {
        // Clear display by drawing black rectangle
        embedded_graphics::primitives::Rectangle::new(
            Point::new(0, 0),
            Size::new(96, 16) // Assuming 96x16 display, adjust as needed
        )
        .into_styled(PrimitiveStyle::with_fill(Rgb888::BLACK))
        .draw(display.target_mut())
        .unwrap();

        // Draw bike inventory
        Text::new(
            &format!("{} Bikes: {}+ {}++", inventory.bikes_available, inventory.ebikes_v1_available, inventory.ebikes_v2_available),
            Point::new(1, display.y_offset),
            MonoTextStyle::new(&FONT_5X7, Rgb888::new(255, 255, 255))
        )
        .draw(display.target_mut())
        .unwrap();

        Text::new(
            &format!("Docks: {}", inventory.docks_available),
            Point::new(1, display.y_offset + 8),
            MonoTextStyle::new(&FONT_5X7, Rgb888::new(255, 255, 255))
        )
        .draw(display.target_mut())
        .unwrap();
    }
}

#[async_trait]
pub trait TransitProvider {
    async fn get_updates(&self, stop: Stop) -> anyhow::Result<TransitState>;
    fn name(&self) -> &'static str;
}

pub mod bart;
pub mod muni;
pub mod baywheels;
pub mod state;
