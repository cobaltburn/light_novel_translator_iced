use async_tempfile::{Ownership, TempFile};
use fs4::tokio::AsyncFileExt as _;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::{fs::OpenOptions, io::AsyncReadExt};

use crate::{error::Result, model::page::Section};

#[derive(Debug, Serialize, Deserialize)]
pub struct Recovery {
    pub file: PathBuf,
    pub pages: Vec<RecoveryPage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryPage {
    pub path: PathBuf,
    pub sections: Vec<Section>,
}

pub async fn recover_files(files: Vec<PathBuf>) -> Vec<Result<(TempFile, Recovery)>> {
    let mut recovery_files = Vec::new();
    for file in files {
        let Ok(probe) = OpenOptions::new().read(true).write(true).open(&file).await else {
            continue;
        };
        if probe.try_lock().is_err() {
            continue;
        }
        drop(probe);

        let Ok(mut temp) = TempFile::from_existing(file.clone(), Ownership::Owned).await else {
            continue;
        };

        if temp.try_lock().is_ok() {
            let mut json = String::new();
            match temp.read_to_string(&mut json).await {
                Ok(_) => {
                    let recovery = serde_json::from_str::<Recovery>(&json)
                        .and_then(|r| Ok((temp, r)))
                        .map_err(Into::into);
                    recovery_files.push(recovery);
                }
                Err(err) => recovery_files.push(Err(err.into())),
            }
        }
    }

    return recovery_files;
}
