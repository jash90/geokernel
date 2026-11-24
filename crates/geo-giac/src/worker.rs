//! One thread owns the Giac library and its context.
//!
//! Giac's context is not safe to share between threads, and the reference
//! serialises its calls too (`callEvaluateFunction` is synchronized). Giac
//! also recurses deeply, so the thread gets a large stack instead of the
//! default one of a test thread.
//!
//! A caller waits at most the evaluation's time limit plus a margin. When
//! that passes, Giac is stuck in a call that cannot be interrupted from
//! outside: the thread keeps running it, every later call returns
//! [`Error::Busy`] at once, and the thread takes calls again as soon as Giac
//! returns (its late answer is dropped). A second copy of the library cannot
//! be loaded beside it, since both would share Giac's global state.

use std::ffi::OsString;
#[cfg(any(test, target_os = "android"))]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::native::Native;
use crate::{session, Error, LIBRARY_VARIABLE};

const STACK_BYTES: usize = 256 << 20;

/// Time a caller allows beyond Giac's own limit (the reset before each
/// evaluation, and Giac checking its limit only every so often).
const MARGIN_MS: u64 = 5000;

enum Job {
    Raw {
        input: String,
        timeout_ms: u64,
        seed: i32,
    },
    Plain(String),
}

type Reply = Sender<Result<String, Error>>;

/// The callers' side of the worker thread.
struct Link<J> {
    sender: Mutex<Sender<(J, Reply)>>,
    /// Set while a call is in flight: by the caller before it sends the
    /// call, cleared by the worker before it replies. A caller that stops
    /// waiting leaves it as it is, so later calls see the worker busy until
    /// the call returns, and never a busy flag left behind by an idle worker.
    in_flight: Arc<AtomicBool>,
}

impl<J> Link<J> {
    fn call(&self, job: J, wait: Duration) -> Result<String, Error> {
        // One call in flight at a time: the lock is held until the answer
        // comes or the wait ends, and makes the check and the store one step.
        let sender = self.sender.lock().map_err(|_| Error::Worker)?;
        if self.in_flight.load(Ordering::SeqCst) {
            return Err(Error::Busy);
        }
        let (reply_tx, reply_rx) = channel();
        self.in_flight.store(true, Ordering::SeqCst);
        if sender.send((job, reply_tx)).is_err() {
            self.in_flight.store(false, Ordering::SeqCst);
            return Err(Error::Worker);
        }
        match reply_rx.recv_timeout(wait) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(Error::Timeout),
            Err(RecvTimeoutError::Disconnected) => Err(Error::Worker),
        }
    }
}

/// The worker's loop: runs each call and clears the flag before replying.
fn serve<J>(
    calls: Receiver<(J, Reply)>,
    in_flight: &AtomicBool,
    mut run: impl FnMut(J) -> Result<String, Error>,
) {
    for (job, reply) in calls {
        let result = run(job);
        in_flight.store(false, Ordering::SeqCst);
        let _ = reply.send(result);
    }
}

struct State {
    backend: (crate::Backend, Option<String>),
    worker: Result<Link<Job>, Error>,
}

static STATE: OnceLock<State> = OnceLock::new();

static SIMULATED: Mutex<Option<Error>> = Mutex::new(None);

pub(crate) fn simulate_failure(error: Option<Error>) {
    if let Ok(mut simulated) = SIMULATED.lock() {
        *simulated = error;
    }
}

/// An explicitly empty override preserves the kernel-only mode. When
/// unset, use only the executable's own directory or its app bundle.
fn library_path(configured: Option<OsString>) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        if path.is_empty() {
            return Err(format!("{LIBRARY_VARIABLE} is empty"));
        }
        return Ok(path.into());
    }
    let name = crate::native::LIBRARY_NAME.ok_or("no bundled Giac library on this platform")?;
    let executable = std::env::current_exe().map_err(|err| err.to_string())?;
    let directory = executable
        .parent()
        .ok_or("executable has no parent directory")?;
    let sibling = directory.join(name);
    if cfg!(target_os = "macos") && !sibling.exists() && directory.ends_with("Contents/MacOS") {
        return Ok(directory.join("../Frameworks").join(name));
    }
    Ok(sibling)
}

#[cfg(any(test, target_os = "android"))]
fn android_library_path(directory: &Path) -> Result<PathBuf, Error> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(Error::Load(
            "expected an absolute Android native library directory".into(),
        ));
    }
    for name in ["libjavagiac.so", "libc++_shared.so"] {
        if !directory.join(name).is_file() {
            return Err(Error::Load(format!(
                "Android native library directory is missing {name}"
            )));
        }
    }
    Ok(directory.join("libjavagiac.so"))
}

#[cfg(target_os = "android")]
pub(crate) fn initialize_from_android_dir(directory: &Path) -> Result<(), Error> {
    let path = android_library_path(directory)?;
    let mut initialized_here = false;
    let state = STATE.get_or_init(|| {
        initialized_here = true;
        State::with_library(path)
    });
    if !initialized_here {
        return Err(Error::AlreadyInitialized);
    }
    state.worker.as_ref().map(|_| ()).map_err(Clone::clone)
}

impl State {
    fn kernel_only(reason: String) -> Self {
        Self {
            backend: (crate::Backend::KernelOnly, Some(reason)),
            worker: Err(Error::NoLibrary),
        }
    }

    fn with_library(path: PathBuf) -> Self {
        let worker = start(path);
        let backend = match &worker {
            Ok(_) => (crate::Backend::Giac, None),
            Err(error) => {
                let reason = error.to_string();
                eprintln!("geo-giac: {reason}; every Giac result is undefined in this process");
                (crate::Backend::Failed, Some(reason))
            }
        };
        Self { backend, worker }
    }
}

fn state() -> &'static State {
    STATE.get_or_init(|| match library_path(std::env::var_os(LIBRARY_VARIABLE)) {
        Ok(path) if path.exists() => State::with_library(path),
        Ok(path) => State::kernel_only(format!("{} does not exist", path.display())),
        Err(reason) => State::kernel_only(reason),
    })
}

pub(crate) fn backend() -> &'static (crate::Backend, Option<String>) {
    &state().backend
}

fn start(path: PathBuf) -> Result<Link<Job>, Error> {
    let (ready_tx, ready_rx) = channel::<Result<(), Error>>();
    let (tx, rx) = channel::<(Job, Reply)>();
    let in_flight = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&in_flight);
    std::thread::Builder::new()
        .name("giac".into())
        .stack_size(STACK_BYTES)
        .spawn(move || {
            let native = match Native::load(&path) {
                Ok(native) => native,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let context = match native.context() {
                Ok(context) => context,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));
            serve(rx, &flag, |job| match job {
                Job::Raw {
                    input,
                    timeout_ms,
                    seed,
                } => session::eval_raw(&context, &input, timeout_ms, seed),
                Job::Plain(text) => context.eval(&text),
            });
        })
        .map_err(|error| Error::Load(error.to_string()))?;
    ready_rx.recv().map_err(|_| Error::Worker)??;
    Ok(Link {
        sender: Mutex::new(tx),
        in_flight,
    })
}

fn run(job: Job) -> Result<String, Error> {
    let link = state().worker.as_ref().map_err(Clone::clone)?;
    if let Some(error) = SIMULATED.lock().ok().and_then(|s| s.clone()) {
        return Err(error);
    }
    let limit = match &job {
        Job::Raw { timeout_ms, .. } => *timeout_ms,
        Job::Plain(_) => session::DEFAULT_TIMEOUT_MS,
    };
    link.call(job, Duration::from_millis(limit.saturating_add(MARGIN_MS)))
}

pub(crate) fn eval_raw(input: &str, timeout_ms: u64, seed: i32) -> Result<String, Error> {
    run(Job::Raw {
        input: input.to_string(),
        timeout_ms,
        seed,
    })
}

pub(crate) fn eval_plain(text: &str) -> Result<String, Error> {
    run(Job::Plain(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_android_libraries_and_refuses_incomplete_directories() {
        assert!(android_library_path(Path::new("relative")).is_err());
        let directory =
            std::env::temp_dir().join(format!("geo-giac-android-{}", std::process::id()));
        assert!(android_library_path(&directory).is_err());
        std::fs::create_dir(&directory).unwrap();
        let giac = directory.join("libjavagiac.so");
        std::fs::write(&giac, b"placeholder").unwrap();
        assert!(
            matches!(android_library_path(&directory), Err(Error::Load(reason)) if reason.ends_with("libc++_shared.so"))
        );
        std::fs::write(directory.join("libc++_shared.so"), b"placeholder").unwrap();
        assert_eq!(android_library_path(&directory), Ok(giac));
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// A worker that sleeps for the job's milliseconds and answers them.
    fn sleeper() -> Link<u64> {
        let (tx, rx) = channel::<(u64, Reply)>();
        let in_flight = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&in_flight);
        std::thread::spawn(move || {
            serve(rx, &flag, |ms| {
                std::thread::sleep(Duration::from_millis(ms));
                Ok(ms.to_string())
            })
        });
        Link {
            sender: Mutex::new(tx),
            in_flight,
        }
    }

    #[test]
    fn a_timed_out_call_makes_the_worker_busy_until_it_returns() {
        let link = sleeper();
        let wait = Duration::from_millis(100);
        assert_eq!(link.call(5, wait), Ok("5".into()));
        assert_eq!(link.call(400, wait), Err(Error::Timeout));
        assert_eq!(link.call(5, wait), Err(Error::Busy));
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(link.call(5, wait), Ok("5".into()));
    }

    #[test]
    fn a_call_that_finishes_right_after_the_wait_leaves_no_busy_flag() {
        // Giac finishing just after the caller gave up used to latch the
        // busy flag on an idle worker.
        let link = sleeper();
        for _ in 0..10 {
            // Either outcome of this call is fine (a late wake-up can still
            // collect the answer); the next call must succeed.
            let _ = link.call(30, Duration::from_millis(29));
            std::thread::sleep(Duration::from_millis(200));
            assert_eq!(link.call(1, Duration::from_millis(1000)), Ok("1".into()));
        }
    }
}
