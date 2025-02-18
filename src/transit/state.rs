use std::collections::HashMap;
use std::error::Error;
use log::debug;
use futures::future::try_join_all;
use crate::config::{Config, Stop};
use super::{TransitIdentifier, TransitProvider, TransitState};

pub async fn get_state_for_stops<'a, T: TransitProvider>(
    provider: &'a T,
    stops: &'a [Stop],
) -> Result<Vec<TransitState>, Box<dyn Error + Send + Sync>> {
    let futures: Vec<_> = stops
        .iter()
        .map(|stop| async move {
            let updates = provider.get_updates(stop.clone()).await?;
            Ok::<_, Box<dyn Error + Send + Sync>>(updates)
        })
        .collect();

    let result = try_join_all(futures).await?;

    Ok(result)
}

pub struct TransitStateManager {
    config: Config,
    pub bart: super::bart::BartProvider,
    pub muni: super::muni::MuniProvider,
    pub baywheels: super::baywheels::BayWheelsProvider,

    pub state: HashMap<TransitIdentifier, TransitState>,
}

impl TransitStateManager {
    pub fn new(config: Config, bart_api_key: String, muni_api_key: String) -> Self {
        Self {
            config,
            bart: super::bart::BartProvider::new(bart_api_key),
            muni: super::muni::MuniProvider::new(muni_api_key),
            baywheels: super::baywheels::BayWheelsProvider::new(),
            state: HashMap::new(),
        }
    }

    pub fn get_state_for_identifier(&self, identifier: &TransitIdentifier) -> TransitState {
        self.state.get(identifier).cloned().unwrap_or_default()
    }

    async fn fetch_all<'a>(&'a self) -> (
        Result<Vec<TransitState>, Box<dyn Error + Send + Sync>>,
        Result<Vec<TransitState>, Box<dyn Error + Send + Sync>>,
        Result<Vec<TransitState>, Box<dyn Error + Send + Sync>>,
    ) {
        tokio::join!(
            async {
                debug!("Getting BART Updates...");
                get_state_for_stops(&self.bart, &self.config.bart.stops).await
            },
            async {
                debug!("Getting Muni Updates...");
                get_state_for_stops(&self.muni, &self.config.muni.stops).await
            },
            async {
                debug!("Getting Bay Wheels Updates...");
                get_state_for_stops(&self.baywheels, &self.config.baywheels.stops).await
            }
        )
    }

    pub async fn update_state(&mut self) -> () {
        let (bart, muni, baywheels) = self.fetch_all().await;
        // Update self.state with new updates
        self.state.extend(bart.unwrap_or_default().iter().flat_map(|state| state.to_state_updates()));
        self.state.extend(muni.unwrap_or_default().iter().flat_map(|state| state.to_state_updates()));
        self.state.extend(baywheels.unwrap_or_default().iter().flat_map(|state| state.to_state_updates()));
    }
}
