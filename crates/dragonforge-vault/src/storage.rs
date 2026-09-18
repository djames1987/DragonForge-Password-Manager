use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{MAX_VAULT_FILE_BYTES, Result, VaultError};

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    recover_if_needed(path)?;
    let metadata = fs::metadata(path).map_err(|source| VaultError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() > MAX_VAULT_FILE_BYTES {
        return Err(VaultError::ResourceLimit(format!(
            "vault file is {} bytes; maximum is {} bytes",
            metadata.len(),
            MAX_VAULT_FILE_BYTES
        )));
    }

    fs::read(path).map_err(|source| VaultError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let byte_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if byte_len > MAX_VAULT_FILE_BYTES {
        return Err(VaultError::ResourceLimit(format!(
            "refusing to write {} bytes; maximum is {} bytes",
            byte_len, MAX_VAULT_FILE_BYTES
        )));
    }

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
        sync_parent(path)?;
    }

    match fs::rename(&tmp, path) {
        Ok(()) => {
            sync_parent(path)?;
            if backup.exists() {
                fs::remove_file(&backup).map_err(|source| VaultError::Io {
                    path: backup.clone(),
                    source,
                })?;
                sync_parent(path)?;
            }
            Ok(())
        }
        Err(source) => {
            if backup.exists() && !path.exists() {
                let _ = fs::rename(&backup, path);
                let _ = sync_parent(path);
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
        sync_parent(path)?;
    }
    Ok(())
}

pub(crate) fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("vault")
    ))
}

pub(crate) fn backup_path(path: &Path) -> PathBuf {
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

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let directory = File::open(parent).map_err(|source| VaultError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    directory.sync_all().map_err(|source| VaultError::Io {
        path: parent.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<()> {
    Ok(())
}
