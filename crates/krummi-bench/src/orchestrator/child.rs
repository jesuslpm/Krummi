//! Child processes (the Voron build and the workers): one at a time, with their output collected, stopped by
//! Ctrl+C (K3, *Validaciones y mensajes*).

use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// How often a running child is checked for its end and for Ctrl+C.
const POLL: Duration = Duration::from_millis(10);

/// How long a child that failed is given for the Ctrl+C that may have ended it to reach this process too: the
/// console sends it to every process attached to it, and the child may see it first.
const GRACE: Duration = Duration::from_millis(200);

/// Makes Ctrl+C set the flag `interrupted` reads instead of ending the process.
pub fn watch_interrupt() {
    let _ = ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::SeqCst));
}

/// Whether Ctrl+C was pressed since `watch_interrupt`.
pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// A child that ran to its end.
pub struct Finished {
    /// The exit code; `None` when a signal ended it.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// How a child ended.
pub enum Ended {
    Finished(Finished),
    /// It could not start.
    NotStarted(io::Error),
    /// Ctrl+C was pressed: the child was killed and waited for.
    Interrupted,
}

/// Runs `command` to its end, without input and with its standard output and error collected.
pub fn run(command: &mut Command) -> Ended {
    if interrupted() {
        return Ended::Interrupted;
    }
    let spawned = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => return Ended::NotStarted(e),
    };
    let stdout = collect(child.stdout.take());
    let stderr = collect(child.stderr.take());
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break child.wait().ok(),
        }
        if interrupted() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = (stdout.join(), stderr.join());
            return Ended::Interrupted;
        }
        thread::sleep(POLL);
    };
    let finished = Finished {
        code: status.and_then(|s| s.code()),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    };
    if finished.code != Some(0) {
        thread::sleep(GRACE);
        if interrupted() {
            return Ended::Interrupted;
        }
    }
    Ended::Finished(finished)
}

/// Reads a child's pipe to its end in a thread of its own, so that a full pipe never blocks the child.
fn collect<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}
