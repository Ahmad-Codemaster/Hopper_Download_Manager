use crate::model::{EngineError, JobId, JobSpec, JobState};
use crate::store::JobStore;
use crate::validation::validate_job_spec;
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::copy;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub destination_root: PathBuf,
}

pub struct TransferEngine {
    store: JobStore,
    destination_root: PathBuf,
    client: Client,
    operational_sources: Mutex<HashMap<JobId, String>>,
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
            operational_sources: Mutex::new(HashMap::new()),
        })
    }

    pub fn create_job(&self, spec: JobSpec) -> Result<JobId, EngineError> {
        validate_job_spec(&spec)?;
        let destination = self.destination_root.join(&spec.destination);
        if destination.exists() && matches!(spec.conflict_policy, crate::model::ConflictPolicy::Reject) {
            return Err(EngineError::DestinationExists);
        }
        let id = self.store.create_job(&spec)?;
        self.operational_sources
            .lock()
            .map_err(|_| EngineError::InvalidJob("source lock poisoned".into()))?
            .insert(id.clone(), spec.source.to_string());
        Ok(id)
    }

    pub fn start_job(&self, id: JobId) -> Result<(), EngineError> {
        self.store.set_state(id.clone(), JobState::Active)?;
        let job = self.store.get_job(id.clone())?;
        let source = self
            .operational_sources
            .lock()
            .map_err(|_| EngineError::InvalidJob("source lock poisoned".into()))?
            .get(&id)
            .cloned()
            .ok_or_else(|| EngineError::InvalidJob("reauthorization required after restart".into()))?;
        let temporary = self.destination_root.join(format!(".{}.part", id.0));
        let mut final_path = self.destination_root.join(job.destination);
        if final_path.exists() && matches!(job.conflict_policy, crate::model::ConflictPolicy::Rename) {
            let stem = final_path.file_stem().and_then(|value| value.to_str()).unwrap_or("download");
            let extension = final_path.extension().and_then(|value| value.to_str()).unwrap_or("");
            let mut index = 1;
            loop {
                let name = if extension.is_empty() {
                    format!("{stem} ({index})")
                } else {
                    format!("{stem} ({index}).{extension}")
                };
                let candidate = final_path.with_file_name(name);
                if !candidate.exists() {
                    final_path = candidate;
                    break;
                }
                index += 1;
            }
        }
        let result = (|| {
            let mut response = self
                .client
                .get(source)
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
