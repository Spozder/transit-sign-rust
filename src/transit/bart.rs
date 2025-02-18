use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::config::Stop;
use super::{Prediction, TransitProvider, TransitState};
use crate::display::Color;

pub struct BartProvider {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct BartResponse {
    root: BartRoot,
}

#[derive(Debug, Deserialize)]
struct BartRoot {
    station: Vec<BartStation>,
}

#[derive(Debug, Deserialize)]
struct BartStation {
    etd: Vec<BartETD>,
}

#[derive(Debug, Deserialize)]
struct BartETD {
    abbreviation: String,
    estimate: Vec<BartEstimate>,
}

#[derive(Debug, Deserialize)]
struct BartEstimate {
    minutes: String,
    direction: String,
    color: String,
}

impl BartProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TransitProvider for BartProvider {
    fn name(&self) -> &'static str {
        "BART"
    }

    async fn get_updates(&self, stop: Stop) -> anyhow::Result<TransitState> {
        let url = format!(
            "https://api.bart.gov/api/etd.aspx?cmd=etd&orig={}&key={}&json=y",
            stop.id, self.api_key
        );

        let response = self.client.get(&url).send().await?;
        let bart_data: BartResponse = response.json().await?;
        
        let mut predictions = Vec::new();
        
        for bart_station in bart_data.root.station {
            for etd in bart_station.etd {
                for estimate in etd.estimate {
                    let minutes = if estimate.minutes == "Leaving" {
                        0
                    } else {
                        estimate.minutes.parse()?
                    };

                    predictions.push(Prediction {
                        provider_key: "bart".to_string(),
                        station_id: stop.id.clone(),
                        route_name: format!("{}", estimate.color),
                        destination: etd.abbreviation.clone(),
                        minutes_until_arrival: minutes,
                        predicted_time: Utc::now() + chrono::Duration::minutes(minutes as i64),
                        stop_id: stop.id.clone(),
                        direction: estimate.direction,
                        color: Color::from_str(&estimate.color).unwrap_or_default(),
                    });
                }
            }
        }

        Ok(TransitState::Predictions(predictions))
    }
}
