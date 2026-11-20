use std::fs::File;
use std::io;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::timeout;

pub struct LocalServer {
    pub url: String,
    child: Child,
    _directory: TempDir,
}

impl LocalServer {
    pub async fn start(raid_floor: Option<u32>) -> io::Result<Self> {
        let client = std::env::current_exe()?;
        let executable = ["hyaku-local-server", "hyaku-server"]
            .into_iter()
            .map(|name| client.with_file_name(name))
            .find(|path| path.is_file())
            .ok_or_else(|| io::Error::new(
                io::ErrorKind::NotFound,
                "local server binary missing; build both packages with cargo build --workspace or reinstall Hyaku",
            ))?;

        let directory = tempfile::Builder::new().prefix("hyaku-local-").tempdir()?;
        let error_log = directory.path().join("server.log");
        let mut command = Command::new(executable);
        command.env_remove("HYAKU_RAID_TEST_FLOOR");
        if let Some(floor) = raid_floor {
            command.env("HYAKU_RAID_TEST_FLOOR", floor.to_string());
        }
        let mut child = command
            .env("HYAKU_BIND_ADDR", "127.0.0.1:0")
            .env("HYAKU_SAVE_PATH", directory.path().join("hyaku_save.json"))
            .env_remove("HYAKU_DEMO")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(File::create(&error_log)?)
            .kill_on_drop(true)
            .spawn()?;
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        let ready = timeout(Duration::from_secs(5), async {
            while let Some(line) = output.next_line().await? {
                if let Some(url) = line.strip_prefix("[HYAKU] Ready. Clients may connect to ") {
                    return Ok(url.to_owned());
                }
            }
            let detail = std::fs::read_to_string(&error_log).unwrap_or_default();
            Err(io::Error::other(format!("local server exited before startup. {}", detail.trim())))
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "local server took too long to start"))??;

        // The server logs to this pipe throughout the session.
        tokio::spawn(async move {
            while let Ok(Some(_)) = output.next_line().await {}
        });
        Ok(Self { url: ready, child, _directory: directory })
    }

    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
    }
}
