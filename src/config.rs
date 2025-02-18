use serde::Deserialize;
use std::fs;
use std::collections::HashMap;
use std::time::Duration;

use crate::transit::TransitIdentifier;
use crate::display::PageDisplayHandler;

#[derive(Debug, Deserialize, Clone)]
pub struct Stop {
    pub id: String,
    pub name: String,
    pub direction: String,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub stops: Vec<Stop>,
    #[serde(skip)]
    pub stops_by_id: HashMap<String, Stop>,
}

impl ProviderConfig {
    fn init(&mut self) {
        self.stops_by_id = self.stops
            .iter()
            .cloned()
            .map(|stop| (stop.id.clone(), stop))
            .collect();
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bart: ProviderConfig,
    pub muni: ProviderConfig,
    pub baywheels: ProviderConfig,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string("config.toml")?;
        let mut config: Config = toml::from_str(&config_str)?;

        config.bart.init();
        config.muni.init();
        config.baywheels.init();

        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub struct DisplayConfig {
    pub message_timeout: Duration,
    pub error_timeout: Duration,
    pub pages: Vec<PageDefinition>,
}

#[derive(Debug, Deserialize)]
pub struct PageDefinition {
    pub subpages: Vec<SubpageDefinition>,
}

#[derive(Debug, Deserialize)]
pub struct SubpageDefinition {
    pub transit_identifier: TransitIdentifier,
    pub page_display_handler_key: String,
}

impl SubpageDefinition {
    pub fn display_handler(&self) -> PageDisplayHandler {
        PageDisplayHandler::from_key(&self.page_display_handler_key)
    }   
}

impl DisplayConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string("display.toml")?;
        
        match toml::from_str::<DisplayConfig>(&config_str) {
            Ok(config) => Ok(config),
            Err(e) => {
                println!("Error parsing display.toml:");
                println!("Error details: {:#?}", e);
                println!("Error message: {}", e);
                
                // Print the TOML content with line numbers for reference
                println!("\nTOML content with line numbers:");
                for (i, line) in config_str.lines().enumerate() {
                    println!("{:3}: {}", i + 1, line);
                }
                
                Err(Box::new(e))
            }
        }
    }
}
