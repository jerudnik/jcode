use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkingDirSetBy {
    Created,
    Resumed,
    Subscribe,
}

impl Session {
    /// Replace the recorded working directory and attribute the lifecycle change.
    pub fn set_recorded_working_dir(&mut self, dir: &str, set_by: WorkingDirSetBy) {
        self.working_dir = Some(dir.to_string());
        self.working_dir_set_by = Some(set_by);
        self.working_dir_set_at = Some(Utc::now());
    }
}
