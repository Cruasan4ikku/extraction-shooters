use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use eft_core::{validate_inventory, InventorySave};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::Mutex};

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(super) struct BackendState {
    pub(super) inventory_file: Arc<PathBuf>,
    pub(super) file_lock: Arc<Mutex<()>>,
}

impl BackendState {
    pub(super) fn new(inventory_file: PathBuf) -> Self {
        Self {
            inventory_file: Arc::new(inventory_file),
            file_lock: Arc::new(Mutex::new(())),
        }
    }
}

pub(super) async fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        tokio::fs::create_dir_all(parent).await?;
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "inventory path has no file name",
            )
        })?
        .to_string_lossy();
    let (temp_path, mut temp_file) = loop {
        let id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let temp_path =
            path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), id));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .await
        {
            Ok(file) => break (temp_path, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    if let Err(error) = temp_file.write_all(contents).await {
        remove_temp_file(&temp_path).await;
        return Err(error);
    }
    if let Err(error) = temp_file.sync_all().await {
        remove_temp_file(&temp_path).await;
        return Err(error);
    }
    drop(temp_file);

    if let Err(error) = tokio::fs::rename(&temp_path, path).await {
        remove_temp_file(&temp_path).await;
        return Err(error);
    }

    Ok(())
}

async fn remove_temp_file(path: &Path) {
    if let Err(error) = tokio::fs::remove_file(path).await {
        if error.kind() != io::ErrorKind::NotFound {
            eprintln!(
                "failed to remove temporary inventory file {}: {error}",
                path.display()
            );
        }
    }
}

pub(super) async fn migrate_legacy_inventory(target: &Path, legacy: &Path) -> io::Result<()> {
    match tokio::fs::metadata(target).await {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let contents = match tokio::fs::read(legacy).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    let save = InventorySave::decode(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    validate_inventory(&save.inventory)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    atomic_write(target, &contents).await
}
