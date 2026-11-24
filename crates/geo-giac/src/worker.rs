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

static WORKER: OnceLock<Result<Link<Job>, Error>> = OnceLock::new();

static BACKEND: OnceLock<(crate::Backend, Option<String>)> = OnceLock::new();

static SIMULATED: Mutex<Option<Error>> = Mutex::new(None);

pub(crate) fn simulate_failure(error: Option<Error>) {
    if let Ok(mut simulated) = SIMULATED.lock() {
        *simulated = error;
    }
}

/// The process's backend, decided at the first call: no configured library
/// means the kernel alone; a configured library that does not load is a
/// failure, reported once.
pub(crate) fn backend() -> &'static (crate::Backend, Option<String>) {
    BACKEND.get_or_init(|| {
        let configured = std::env::var_os(LIBRARY_VARIABLE).filter(|path| !path.is_empty());
        let Some(path) = configured else {
            return (
                crate::Backend::KernelOnly,
                Some(format!("{LIBRARY_VARIABLE} is not set")),
            );
        };
        if !std::path::Path::new(&path).exists() {
            let reason = format!(
                "{LIBRARY_VARIABLE} names {}, which does not exist",
                PathBuf::from(&path).display()
            );
            return (crate::Backend::KernelOnly, Some(reason));
        }
        match WORKER.get_or_init(start) {
            Ok(_) => (crate::Backend::Giac, None),
            Err(error) => {
                let reason = error.to_string();
                eprintln!("geo-giac: {reason}; every Giac result is undefined in this process");
                (crate::Backend::Failed, Some(reason))
            }
        }
    })
}

fn start() -> Result<Link<Job>, Error> {
    let path = std::env::var_os(LIBRARY_VARIABLE)
        .map(PathBuf::from)
        .ok_or(Error::NoLibrary)?;
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
    match backend() {
        (crate::Backend::Giac, _) => {}
        (crate::Backend::KernelOnly, _) => return Err(Error::NoLibrary),
        (crate::Backend::Failed, reason) => {
            return Err(Error::Load(reason.clone().unwrap_or_default()))
        }
    }
    if let Some(error) = SIMULATED.lock().ok().and_then(|s| s.clone()) {
        return Err(error);
    }
    let link = WORKER.get_or_init(start).as_ref().map_err(Clone::clone)?;
    let limit = match &job {
        Job::Raw { timeout_ms, .. } => *timeout_ms,
        Job::Plain(_) => session::DEFAULT_TIMEOUT_MS,
    };
    link.call(job, Duration::from_millis(limit.saturating_add(MARGIN_MS)))
}

// The worker never copies or dispatches an arbitrarily large caller string.
const MAX_INPUT_BYTES: usize = 64 * 1024;

fn check_input(text: &str) -> Result<(), Error> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Error::Call("Giac input is too long".to_owned()));
    }
    Ok(())
}

pub(crate) fn eval_raw(input: &str, timeout_ms: u64, seed: i32) -> Result<String, Error> {
    check_input(input)?;
    run(Job::Raw {
        input: input.to_string(),
        timeout_ms,
        seed,
    })
}

pub(crate) fn eval_plain(text: &str) -> Result<String, Error> {
    check_input(text)?;
    run(Job::Plain(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_inputs_are_refused_before_a_worker_starts() {
        let input = "x".repeat(65_537);
        for result in [
            eval_raw(&input, session::DEFAULT_TIMEOUT_MS, 1),
            eval_plain(&input),
        ] {
            assert!(
                matches!(result, Err(Error::Call(message)) if message.contains("input is too long"))
            );
        }
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
