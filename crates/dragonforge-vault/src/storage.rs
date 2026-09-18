use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{Result, VaultError};

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    recover_if_needed(path)?;
    fs::read(path).map_err(|source| VaultError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| VaultError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let tmp = temporary_path(path);
    let backup = backup_path(path);

    {
        let mut file = secure_create(&tmp)?;
        file.write_all(bytes).map_err(|source| VaultError::Io {
            path: tmp.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| VaultError::Io {
            path: tmp.clone(),
            source,
        })?;
    }

    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|source| VaultError::Io {
                path: backup.clone(),
                source,
            })?;
        }
        fs::rename(path, &backup).map_err(|source| VaultError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }

    match fs::rename(&tmp, path) {
        Ok(()) => {
            if backup.exists() {
                let _ = fs::remove_file(&backup);
            }
            Ok(())
        }
        Err(source) => {
            if backup.exists() && !path.exists() {
                let _ = fs::rename(&backup, path);
            }
            Err(VaultError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    }
}

fn recover_if_needed(path: &Path) -> Result<()> {
    let backup = backup_path(path);
    if !path.exists() && backup.exists() {
        fs::rename(&backup, path).map_err(|source| VaultError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("vault")
    ))
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.bak",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("vault")
    ))
}

fn secure_create(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path).map_err(|source| VaultError::Io {
        path: path.to_path_buf(),
        source,
    })
}
