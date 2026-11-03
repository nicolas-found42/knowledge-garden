//! Bounded local visual interpretation. Results are unverified model interpretations.
use base64::{engine::general_purpose::STANDARD, Engine};
use fs2::FileExt;
use reqwest::{
    blocking::{Client, Response},
    redirect::Policy,
    Url,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(unix)]
use std::os::unix::{
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawFd,
    process::CommandExt,
};
use std::{
    fs::OpenOptions,
    io::Read,
    net::{IpAddr, TcpListener},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use thiserror::Error;

const MODEL: &str = "qwen2.5vl:7b";
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 128 * 1024;
static INFERENCE: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualInterpretation {
    pub description: String,
    pub model: String,
    /// Digest observed before and after inference; Ollama does not attest it in the response.
    pub observed_revision: String,
}

#[derive(Debug, Error)]
pub enum VisualError {
    #[error("The local endpoint must be an HTTP loopback address without credentials or a path")]
    InvalidEndpoint,
    #[error("The image must contain between 1 byte and 8 MiB")]
    InvalidImage,
    #[error("Another visual inference is running")]
    Busy,
    #[error("The local model is unavailable")]
    Unavailable,
    #[error("The local model returned an invalid or incomplete response")]
    InvalidResponse,
}

pub struct LocalVisualModel {
    client: Client,
    endpoint: Url,
}

#[derive(Deserialize)]
struct Tags {
    models: Vec<Tag>,
}
#[derive(Deserialize)]
struct Tag {
    name: String,
    digest: String,
}
#[derive(Deserialize)]
struct Generation {
    model: String,
    response: String,
    done: bool,
    done_reason: Option<String>,
}

impl LocalVisualModel {
    pub fn new(endpoint: &str) -> Result<Self, VisualError> {
        Self::with_timeout(endpoint, Duration::from_secs(120))
    }

    fn with_timeout(endpoint: &str, timeout: Duration) -> Result<Self, VisualError> {
        let endpoint = Url::parse(endpoint).map_err(|_| VisualError::InvalidEndpoint)?;
        let loopback = endpoint
            .host_str()
            .and_then(|host| host.parse::<IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if endpoint.scheme() != "http"
            || !loopback
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.path() != "/"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(VisualError::InvalidEndpoint);
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|_| VisualError::Unavailable)?;
        Ok(Self { client, endpoint })
    }

    pub fn interpret(&self, image: &[u8]) -> Result<VisualInterpretation, VisualError> {
        if image.is_empty() || image.len() > MAX_IMAGE_BYTES {
            return Err(VisualError::InvalidImage);
        }
        let _guard = INFERENCE.try_lock().map_err(|_| VisualError::Busy)?;
        let revision = self.observed_revision()?;
        let request = json!({"model": MODEL, "prompt": "Describe only what is visibly present in this image. State uncertainty. Do not infer relationships to unseen events.", "images": [STANDARD.encode(image)], "stream": false, "options": {"num_predict": 256}});
        let result: Generation = self.read(
            self.client
                .post(self.endpoint.join("api/generate").unwrap())
                .json(&request)
                .send()
                .map_err(|_| VisualError::Unavailable)?,
        )?;
        if result.model != MODEL
            || !result.done
            || result.done_reason.as_deref() != Some("stop")
            || result.response.trim().is_empty()
            || result.response.len() > 8192
            || self.observed_revision()? != revision
        {
            return Err(VisualError::InvalidResponse);
        }
        Ok(VisualInterpretation {
            description: result.response,
            model: MODEL.into(),
            observed_revision: revision,
        })
    }

    fn observed_revision(&self) -> Result<String, VisualError> {
        let tags: Tags = self.read(
            self.client
                .get(self.endpoint.join("api/tags").unwrap())
                .send()
                .map_err(|_| VisualError::Unavailable)?,
        )?;
        tags.models
            .into_iter()
            .find(|tag| {
                tag.name == MODEL
                    && tag.digest.len() == 64
                    && tag.digest.bytes().all(|b| b.is_ascii_hexdigit())
            })
            .map(|tag| tag.digest)
            .ok_or(VisualError::Unavailable)
    }

    fn read<T: serde::de::DeserializeOwned>(&self, response: Response) -> Result<T, VisualError> {
        if !response.status().is_success() {
            return Err(VisualError::Unavailable);
        }
        let mut body = Vec::new();
        response
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|_| VisualError::Unavailable)?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(VisualError::InvalidResponse);
        }
        serde_json::from_slice(&body).map_err(|_| VisualError::InvalidResponse)
    }
}

/// Starts a dedicated local server for one request and holds an inter-process lease until
/// its process group is gone. This is not yet wired to source processing.
#[cfg(unix)]
pub struct ManagedVisualModel {
    executable: PathBuf,
    lease_path: PathBuf,
    timeout: Duration,
}

#[cfg(unix)]
impl ManagedVisualModel {
    pub fn system() -> Self {
        let uid = unsafe { libc::geteuid() };
        Self::new(
            "/usr/local/bin/ollama",
            format!("/tmp/knowledge-garden-visual-{uid}.lock"),
        )
    }

    pub fn new(executable: impl AsRef<Path>, lease_path: impl AsRef<Path>) -> Self {
        Self {
            executable: executable.as_ref().to_owned(),
            lease_path: lease_path.as_ref().to_owned(),
            timeout: Duration::from_secs(120),
        }
    }

    pub fn interpret_file(&self, path: &Path) -> Result<VisualInterpretation, VisualError> {
        let file = std::fs::File::open(path).map_err(|_| VisualError::InvalidImage)?;
        let mut image = Vec::new();
        file.take(MAX_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut image)
            .map_err(|_| VisualError::InvalidImage)?;
        self.interpret(&image)
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, VisualError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(VisualError::Unavailable);
        }
        self.timeout = timeout;
        Ok(self)
    }

    pub fn interpret(&self, image: &[u8]) -> Result<VisualInterpretation, VisualError> {
        if image.is_empty() || image.len() > MAX_IMAGE_BYTES {
            return Err(VisualError::InvalidImage);
        }
        let lease = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.lease_path)
            .map_err(|_| VisualError::Unavailable)?;
        let meta = lease.metadata().map_err(|_| VisualError::Unavailable)?;
        if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } {
            return Err(VisualError::Unavailable);
        }
        lease.try_lock_exclusive().map_err(|_| VisualError::Busy)?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| VisualError::Unavailable)?;
        let address = listener
            .local_addr()
            .map_err(|_| VisualError::Unavailable)?;
        drop(listener);
        // The supervisor stays process-group leader until its parent-held pipe closes.
        // A crashed application closes the pipe too, so the supervisor kills its group.
        let mut command = Command::new("/bin/sh");
        let lease_fd = lease.as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                let flags = libc::fcntl(lease_fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(lease_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .arg("-c")
            .arg("\"$1\" serve </dev/null & /bin/cat >/dev/null; /bin/kill -KILL -- -$$")
            .arg("kg-visual-supervisor")
            .arg(&self.executable)
            .env("OLLAMA_HOST", address.to_string())
            .env("OLLAMA_NUM_PARALLEL", "1")
            .env("OLLAMA_MAX_LOADED_MODELS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|_| VisualError::Unavailable)?;
        let pipe = child.stdin.take().ok_or(VisualError::Unavailable)?;
        let mut server = OwnedServer {
            child: Some(child),
            pipe: Some(pipe),
        };
        let endpoint = format!("http://{address}");
        let startup = LocalVisualModel::with_timeout(&endpoint, Duration::from_millis(300))?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if server
                .child
                .as_mut()
                .unwrap()
                .try_wait()
                .map_err(|_| VisualError::Unavailable)?
                .is_some()
            {
                return Err(VisualError::Unavailable);
            }
            if startup.observed_revision().is_ok() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(VisualError::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let model = LocalVisualModel::with_timeout(&endpoint, self.timeout)?;
        let result = model.interpret(image);
        server.shutdown()?;
        drop(lease);
        result
    }
}

#[cfg(unix)]
struct OwnedServer {
    child: Option<Child>,
    pipe: Option<ChildStdin>,
}
#[cfg(unix)]
impl OwnedServer {
    fn shutdown(&mut self) -> Result<(), VisualError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let group = -(child.id() as libc::pid_t);
        drop(self.pipe.take());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if child
                .try_wait()
                .map_err(|_| VisualError::Unavailable)?
                .is_some()
            {
                break;
            }
            if Instant::now() >= deadline {
                unsafe {
                    libc::kill(group, libc::SIGKILL);
                }
                let _ = child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let status = unsafe { libc::kill(group, 0) };
            if status == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err(VisualError::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
#[cfg(unix)]
impl Drop for OwnedServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
