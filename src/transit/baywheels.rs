use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::config::Stop;
use super::{BikeInventory, TransitProvider, TransitState};

const EBIKES_API_URL: &str = "https://gbfs.lyft.com/gbfs/1.1/bay/fr/ebikes_at_stations.json";
const STATION_STATUS_URL: &str = "https://gbfs.lyft.com/gbfs/1.1/bay/en/station_status.json";

pub struct BayWheelsProvider {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct EbikeResponse {
    data: EbikeData,
}

#[derive(Debug, Deserialize)]
struct EbikeData {
    stations: Vec<EbikeStation>,
}

#[derive(Debug, Deserialize)]
struct EbikeStation {
    station_id: String,
    ebikes: Vec<Ebike>,
}

#[derive(Debug, Deserialize)]
struct Ebike {
    make_and_model: String,
    battery_charge_percentage: i32,
}

#[derive(Debug, Deserialize)]
struct StationStatusResponse {
    data: StationStatusData,
}

#[derive(Debug, Deserialize)]
struct StationStatusData {
    stations: Vec<StationStatus>,
}

#[derive(Debug, Deserialize)]
struct StationStatus {
    station_id: String,
    num_bikes_available: i32,
    num_docks_available: i32,
}

impl BayWheelsProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn count_ebikes_by_model(ebikes: &[Ebike]) -> (i32, i32) {
        let mut v1_count = 0;
        let mut v2_count = 0;

        for ebike in ebikes {
            match ebike.make_and_model.as_str() {
                "lyft_bike_watson" => v1_count += 1,
                "lyft_bike_cosmo" => v2_count += 1,
                _ => (), // Unknown model
            }
        }

        (v1_count, v2_count)
    }
}

#[async_trait]
impl TransitProvider for BayWheelsProvider {
    fn name(&self) -> &'static str {
        "Bay Wheels"
    }

    async fn get_updates(&self, stop: Stop) -> anyhow::Result<TransitState> {
        // Fetch both ebike and station status data concurrently
        let client = self.client.clone();
        let ebike_handle = tokio::spawn(async move {
            let resp = client.get(EBIKES_API_URL).send().await?;
            resp.json::<EbikeResponse>().await
        });
        
        let client = self.client.clone();
        let status_handle = tokio::spawn(async move {
            let resp = client.get(STATION_STATUS_URL).send().await?;
            resp.json::<StationStatusResponse>().await
        });

        // Wait for both requests to complete and handle errors
        let ebike_response = ebike_handle.await.map_err(|e| anyhow::anyhow!("Task failed: {}", e))??;
        let status_response = status_handle.await.map_err(|e| anyhow::anyhow!("Task failed: {}", e))??;

        // Get station status first
        let station_status = status_response.data.stations
            .iter()
            .find(|s| s.station_id == stop.id)
            .ok_or_else(|| anyhow::anyhow!("Station not found: {}", stop.id))?;

        // Get ebike data if available
        let ebike_station_id = format!("motivate_SFO_{}", stop.id);
        let (ebikes_v1, ebikes_v2) = if let Some(ebike_station) = ebike_response.data.stations
            .iter()
            .find(|s| s.station_id == ebike_station_id) {
            Self::count_ebikes_by_model(&ebike_station.ebikes)
        } else {
            (0, 0) // No ebike data available for this station
        };
        
        Ok(TransitState::BikeInventory(BikeInventory {
            station_name: stop.id.clone(),
            bikes_available: station_status.num_bikes_available,
            docks_available: station_status.num_docks_available,
            ebikes_v1_available: ebikes_v1,
            ebikes_v2_available: ebikes_v2,
            last_updated: Utc::now(),
            stop_id: stop.id.clone()
        }))
    }
}
