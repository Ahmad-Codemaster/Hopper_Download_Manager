use crate::model::{EngineError, JobId, JobSpec, JobState};
use crate::store::JobStore;
use crate::validation::validate_job_spec;
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::copy;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub destination_root: PathBuf,
}

pub struct TransferEngine {
    store: JobStore,
    destination_root: PathBuf,
    client: Client,
}

impl TransferEngine {
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        fs::create_dir_all(&config.destination_root)
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        let store = JobStore::open(&config.destination_root.join("jobs.db"))?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        Ok(Self {
            store,
            destination_root: config.destination_root,
            client,
        })
    }

    pub fn create_job(&self, spec: JobSpec) -> Result<JobId, EngineError> {
        validate_job_spec(&spec)?;
        let destination = self.destination_root.join(&spec.destination);
        if destination.exists() && matches!(spec.conflict_policy, crate::model::ConflictPolicy::Reject) {
            return Err(EngineError::DestinationExists);
        }
        self.store.create_job(&spec)
    }

    pub fn start_job(&self, id: JobId) -> Result<(), EngineError> {
        self.store.set_state(id.clone(), JobState::Active)?;
        let job = self.store.get_job(id.clone())?;
        let temporary = self.destination_root.join(format!(".{}.part", id.0));
        let final_path = self.destination_root.join(job.destination);
        let result = (|| {
            let mut response = self
                .client
                .get(&job.source)
                .send()
                .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
            if !response.status().is_success() {
                return Err(EngineError::InvalidJob(format!("HTTP {}", response.status())));
            }
            let mut file = File::create(&temporary)
                .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
            let bytes = copy(&mut response, &mut file)
                .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
            self.store.update_checkpoint(id.clone(), bytes)?;
            fs::rename(&temporary, &final_path)
                .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
            self.store.set_state(id.clone(), JobState::Completed)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.store.set_state(id, JobState::Failed);
        }
        result
    }
}
