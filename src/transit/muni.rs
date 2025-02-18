use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::config::Stop;
use super::{Prediction, TransitProvider, TransitState};
use crate::display::Color;

pub struct MuniProvider {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct SiriResponse {
    ServiceDelivery: ServiceDelivery,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct ServiceDelivery {
    ResponseTimestamp: String,
    ProducerRef: String,
    Status: bool,
    StopMonitoringDelivery: StopMonitoringDelivery,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct StopMonitoringDelivery {
    version: String,
    ResponseTimestamp: String,
    Status: bool,
    MonitoredStopVisit: Vec<MonitoredStopVisit>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct MonitoredStopVisit {
    RecordedAtTime: String,
    MonitoringRef: String,
    MonitoredVehicleJourney: MonitoredVehicleJourney,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct MonitoredVehicleJourney {
    LineRef: String,
    DirectionRef: String,
    FramedVehicleJourneyRef: FramedVehicleJourneyRef,
    PublishedLineName: String,
    OperatorRef: String,
    OriginRef: String,
    OriginName: String,
    DestinationRef: String,
    DestinationName: String,
    Monitored: bool,
    InCongestion: Option<bool>,
    VehicleLocation: Option<VehicleLocation>,
    Bearing: Option<String>,
    Occupancy: Option<String>,
    VehicleRef: Option<String>,
    MonitoredCall: MonitoredCall,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct VehicleLocation {
    #[serde(default)]
    Longitude: String,
    #[serde(default)]
    Latitude: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct FramedVehicleJourneyRef {
    DataFrameRef: String,
    DatedVehicleJourneyRef: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct MonitoredCall {
    StopPointRef: String,
    StopPointName: String,
    #[serde(default)]
    VehicleLocationAtStop: String,
    #[serde(default)]
    VehicleAtStop: String,
    DestinationDisplay: String,
    AimedArrivalTime: String,
    ExpectedArrivalTime: String,
    AimedDepartureTime: String,
    ExpectedDepartureTime: Option<String>,
    #[serde(default)]
    Distances: String,
}

impl MuniProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TransitProvider for MuniProvider {
    fn name(&self) -> &'static str {
        "Muni"
    }

    async fn get_updates(&self, stop: Stop) -> anyhow::Result<TransitState> {
        let url = format!(
            "https://api.511.org/transit/StopMonitoring?api_key={}&agency=SF&stopCode={}&format=json",
            self.api_key, stop.id
        );

        let response = self.client.get(&url)
            .header("accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Muni API returned error status: {}",
                response.status()
            ));
        }

        let bytes = response.bytes().await?;
        
        // Convert to string and remove BOM if present
        let response_text = String::from_utf8_lossy(&bytes);
        let cleaned_text = response_text
            .strip_prefix('\u{FEFF}')
            .unwrap_or(&response_text)
            .trim();
        
        let siri_data: SiriResponse = serde_json::from_str(cleaned_text)?;
        let mut predictions = Vec::new();
        
        for visit in &siri_data.ServiceDelivery.StopMonitoringDelivery.MonitoredStopVisit {
            let journey = &visit.MonitoredVehicleJourney;
            let arrival_time = chrono::DateTime::parse_from_rfc3339(&journey.MonitoredCall.ExpectedArrivalTime)?;
            let now = Utc::now();
            let duration = arrival_time.signed_duration_since(now);
            let minutes = duration.num_minutes();
            
            predictions.push(Prediction {
                provider_key: "muni".to_string(),
                station_id: stop.id.clone(),
                route_name: format!("{}", journey.LineRef.trim_start_matches("SF:")),
                destination: journey.DestinationName.clone(),
                minutes_until_arrival: minutes as i32,
                predicted_time: arrival_time.with_timezone(&Utc),
                stop_id: stop.id.clone(),
                direction: stop.direction.clone(),
                color: Color::from_str("PURPLE").unwrap_or_default()
            });
        }

        Ok(TransitState::Predictions(predictions))
    }
}
