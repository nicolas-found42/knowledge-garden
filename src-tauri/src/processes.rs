//! App-owned helper processes. Shutdown never needs the collection mutex.
use std::{
    io::{self, Read},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::{Arc, Mutex, OnceLock, Weak},
    thread,
    time::Duration,
};
#[derive(Default)]
struct Registry {
    stopping: bool,
    children: Vec<Weak<Mutex<Child>>>,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
pub struct Helper(Arc<Mutex<Child>>);
pub fn spawn(command: &mut Command) -> io::Result<Helper> {
    let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
    if registry.stopping {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Application is quitting.",
        ));
    }
    let child = Arc::new(Mutex::new(command.spawn()?));
    registry.children.retain(|child| child.strong_count() > 0);
    registry.children.push(Arc::downgrade(&child));
    Ok(Helper(child))
}
pub fn output(command: &mut Command) -> io::Result<Output> {
    spawn(command.stdout(Stdio::piped()).stderr(Stdio::piped()))?.wait_with_output()
}
pub fn stopping() -> bool {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .stopping
}
pub fn shutdown() {
    let children = {
        let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.stopping = true;
        registry
            .children
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>()
    };
    for child in children {
        let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}
impl Helper {
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).try_wait()
    }
    pub fn kill(&mut self) -> io::Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).kill()
    }
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        fn drain(
            pipe: Option<impl Read + Send + 'static>,
        ) -> thread::JoinHandle<io::Result<Vec<u8>>> {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                if let Some(mut pipe) = pipe {
                    pipe.read_to_end(&mut bytes)?;
                }
                Ok(bytes)
            })
        }
        let (stdout, stderr) = {
            let mut child = self.0.lock().unwrap_or_else(|e| e.into_inner());
            (child.stdout.take(), child.stderr.take())
        };
        let stdout = drain(stdout);
        let stderr = drain(stderr);
        let status = self.wait()?;
        let join = |thread: thread::JoinHandle<io::Result<Vec<u8>>>| {
            thread
                .join()
                .map_err(|_| io::Error::other("Helper output reader stopped."))?
        };
        Ok(Output {
            status,
            stdout: join(stdout)?,
            stderr: join(stderr)?,
        })
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        let mut child = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controlled_shutdown_reaps_helpers_and_rejects_late_spawns() {
        // The stop bit is process-wide, so isolate this lifecycle in a subprocess.
        if std::env::var_os("KG_HELPER_SHUTDOWN_CHILD").is_none() {
            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "processes::tests::controlled_shutdown_reaps_helpers_and_rejects_late_spawns",
                    "--nocapture",
                ])
                .env("KG_HELPER_SHUTDOWN_CHILD", "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        use crate::{
            application::{AcquisitionMethod, Application},
            extraction::LocalExtractor,
            media::{AudioInfo, AudioProcessor, TranscriptionBatch},
            providers::JevSemanticProvider,
        };
        use std::{path::Path, sync::mpsc};
        struct BlockingAudio(mpsc::Sender<()>);
        impl AudioProcessor for BlockingAudio {
            fn inspect(&self, _: &Path) -> Result<AudioInfo, String> {
                Ok(AudioInfo {
                    format: "wav".into(),
                    duration_ms: 1000,
                    sample_rate: 16000.0,
                    channels: 1,
                    codec: "PCM".into(),
                })
            }
            fn install_assets(&self) -> Result<(), String> {
                Ok(())
            }
            fn transcribe_segment(
                &self,
                _: &Path,
                _: u64,
                _: u64,
            ) -> Result<TranscriptionBatch, String> {
                let mut helper = spawn(Command::new("/bin/sleep").arg("60")).unwrap();
                self.0.send(()).unwrap();
                helper.wait().unwrap();
                Err("Cancelled helper".into())
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let (tx, rx) = mpsc::channel();
        let mut app = Application::open_with_all_providers(
            &root,
            Arc::new(JevSemanticProvider::from_environment_and_keychain()),
            Arc::new(LocalExtractor),
            Arc::new(BlockingAudio(tx)),
        )
        .unwrap();
        let source = app
            .import_source(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/heldout-spoken-acts.wav"),
                AcquisitionMethod::Picker,
            )
            .unwrap();
        let worker = thread::spawn(move || {
            assert!(app.resume_due_audio_jobs().is_err());
            app
        });
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut helper = spawn(Command::new("/bin/sleep").arg("60")).unwrap();
        Application::record_controlled_shutdown(&root).unwrap();
        shutdown();
        let app = worker.join().unwrap();
        let pending = app.open_source(&source.info.source_id).unwrap();
        assert_eq!(
            pending.info.audio_processing.as_ref().unwrap().state,
            "processing"
        );
        drop(app);
        let app = Application::open(&root).unwrap();
        let resumed = app.open_source(&source.info.source_id).unwrap();
        let audio = resumed.info.audio_processing.as_ref().unwrap();
        assert_eq!(audio.state, "pending");
        assert_eq!(audio.processing_interruptions, 0);
        assert_eq!(audio.next_start_ms, 0);
        assert!(
            helper.try_wait().unwrap().is_some(),
            "Quit must reap the running helper before exiting"
        );
        assert!(
            spawn(Command::new("/bin/sleep").arg("60")).is_err(),
            "No helper can start after shutdown"
        );
    }
}
