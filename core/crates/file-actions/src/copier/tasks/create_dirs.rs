use std::{path::{Path, PathBuf}, fmt};

use heavy_lifting::{
    task::{Task, TaskId},
    Error,
};
use sd_utils::error::FileIOError;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct CreateDirsTask {
    id: TaskId,
    source_path: PathBuf,
    target_path: PathBuf,
}

impl CreateDirsTask {
    pub fn new(source_path: impl Into<PathBuf>, target_path: impl Into<PathBuf>) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_path: source_path.into(),
            target_path: target_path.into(),
        }
    }
}

#[async_trait::async_trait]
impl Task for CreateDirsTask {
    fn id(&self) -> TaskId {
        self.id
    }

    async fn run(&self) -> Result<(), Error> {
        fs::create_dir_all(&self.target_path)
            .await
            .map_err(|e| FileIOError::from((self.target_path.clone(), e)))?;

        Ok(())
    }

    fn weight(&self) -> u32 {
        1
    }
}
