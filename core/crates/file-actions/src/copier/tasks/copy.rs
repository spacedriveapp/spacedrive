use std::{path::{Path, PathBuf}, fmt};

use heavy_lifting::{
    task::{Task, TaskId},
    Error,
};
use sd_utils::error::FileIOError;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct CopyTask {
    id: TaskId,
    files: Vec<(PathBuf, PathBuf, u64)>, // (source_path, target_path, size)
}

impl CopyTask {
    pub fn new(files: Vec<(PathBuf, PathBuf, u64)>) -> Self {
        Self {
            id: Uuid::new_v4(),
            files,
        }
    }

    async fn find_available_name(path: impl AsRef<Path>) -> Result<PathBuf, Error> {
        let path = path.as_ref();
        
        if !fs::try_exists(path).await.map_err(|e| FileIOError::from((path, e)))? {
            return Ok(path.to_owned());
        }

        let file_stem = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::InvalidPath("File has no valid stem".into()))?;
            
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let parent = path.parent()
            .ok_or_else(|| Error::InvalidPath("File has no parent directory".into()))?;

        for i in 1.. {
            let new_name = if extension.is_empty() {
                format!("{} ({})", file_stem, i)
            } else {
                format!("{} ({}).{}", file_stem, i, extension)
            };

            let new_path = parent.join(new_name);
            if !fs::try_exists(&new_path).await.map_err(|e| FileIOError::from((new_path.clone(), e)))? {
                return Ok(new_path);
            }
        }

        Err(Error::InvalidPath("Could not find available filename".into()))
    }
}

#[async_trait::async_trait]
impl Task for CopyTask {
    fn id(&self) -> TaskId {
        self.id
    }

    async fn run(&self) -> Result<(), Error> {
        for (source, target, _) in &self.files {
            let target = Self::find_available_name(target).await?;
            fs::copy(source, &target)
                .await
                .map_err(|e| FileIOError::from((source.clone(), e)))?;
        }

        Ok(())
    }

    fn weight(&self) -> u32 {
        // Weight is proportional to total size of files to copy
        (self.files.iter().map(|(_, _, size)| size).sum::<u64>() / 1_000_000) as u32 + 1
    }
}
