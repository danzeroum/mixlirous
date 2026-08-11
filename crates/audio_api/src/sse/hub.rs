#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JobEvent {
    pub job_id: Uuid,
    pub event_type: String,
    pub data: serde_json::Value,
}

#[derive(Clone)]
pub struct EventHub {
    channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<JobEvent>>>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, job_id: Uuid) -> broadcast::Receiver<JobEvent> {
        let mut channels = self.channels.write().await;
        match channels.get(&job_id) {
            Some(tx) => tx.subscribe(),
            None => {
                let (tx, rx) = broadcast::channel(200);
                channels.insert(job_id, tx);
                rx
            },
        }
    }

    pub async fn publish(&self, job_id: Uuid, event_type: &str, data: serde_json::Value) {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(&job_id) {
            let event = JobEvent {
                job_id,
                event_type: event_type.to_string(),
                data,
            };
            let _ = tx.send(event);
        }
    }

    pub async fn cleanup(&self, job_id: Uuid) {
        let mut channels = self.channels.write().await;
        channels.remove(&job_id);
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}
