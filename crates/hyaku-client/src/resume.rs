use hyaku_protocol::ResumeKey;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn for_server(server: &str) -> io::Result<ResumeKey> {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            let home = PathBuf::from(env::var_os("HOME")?);
            let dir = if cfg!(target_os = "macos") {
                "Library/Application Support"
            } else {
                ".local/share"
            };
            Some(home.join(dir))
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no user data directory"))?;
    let directory = base.join("hyaku");
    fs::create_dir_all(&directory)?;
    let seed = read_or_create(&directory.join("resume.key"))?;
    Ok(derive_key(&seed, server))
}

fn derive_key(seed: &[u8; 32], server: &str) -> ResumeKey {
    let mut hash = Sha256::new();
    hash.update(b"hyaku resume key v1\0");
    hash.update(server.as_bytes());
    hash.update([0]);
    hash.update(seed);
    ResumeKey(format!("{:x}", hash.finalize()))
}

fn read_or_create(path: &Path) -> io::Result<[u8; 32]> {
    let read = || {
        fs::read(path)?.try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid resume key"))
    };
    match read() {
        Ok(seed) => return Ok(seed),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let mut seed = [0; 32];
    getrandom::fill(&mut seed).map_err(|error| io::Error::other(error.to_string()))?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(directory)?;
    temp.write_all(&seed)?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(path) {
        Ok(_) => Ok(seed),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => read(),
        Err(error) => Err(error.error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_reused_on_restart_and_scoped_to_one_server() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("resume.key");
        let seed = read_or_create(&path).unwrap();
        assert_eq!(read_or_create(&path).unwrap(), seed);
        assert_ne!(derive_key(&seed, "ws://tower"), derive_key(&seed, "ws://other"));
    }
}
