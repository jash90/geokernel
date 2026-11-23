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
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::native::Native;
use crate::{session, Error, LIBRARY_VARIABLE};

const STACK_BYTES: usize = 256 << 20;

/// Time a caller allows beyond Giac's own limit (the reset before each
/// evaluation, and Giac checking its limit only every so often).
const MARGIN_MS: u64 = 5000;

/// Set while the worker runs a call whose caller stopped waiting.
static STUCK: AtomicBool = AtomicBool::new(false);

enum Job {
    Raw {
        input: String,
        timeout_ms: u64,
        seed: i32,
    },
    Plain(String),
}

type Request = (Job, Sender<Result<String, Error>>);

static WORKER: OnceLock<Result<Mutex<Sender<Request>>, Error>> = OnceLock::new();

fn start() -> Result<Mutex<Sender<Request>>, Error> {
    let path = std::env::var_os(LIBRARY_VARIABLE)
        .map(PathBuf::from)
        .ok_or(Error::NoLibrary)?;
    let (ready_tx, ready_rx) = channel::<Result<(), Error>>();
    let (tx, rx) = channel::<Request>();
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
            for (job, reply) in rx {
                let result = match job {
                    Job::Raw {
                        input,
                        timeout_ms,
                        seed,
                    } => session::eval_raw(&context, &input, timeout_ms, seed),
                    Job::Plain(text) => context.eval(&text),
                };
                let _ = reply.send(result);
                STUCK.store(false, Ordering::SeqCst);
            }
        })
        .map_err(|error| Error::Load(error.to_string()))?;
    ready_rx.recv().map_err(|_| Error::Worker)??;
    Ok(Mutex::new(tx))
}

fn run(job: Job) -> Result<String, Error> {
    let sender = WORKER.get_or_init(start).as_ref().map_err(Clone::clone)?;
    let limit = match &job {
        Job::Raw { timeout_ms, .. } => *timeout_ms,
        Job::Plain(_) => session::DEFAULT_TIMEOUT_MS,
    };
    // One call in flight at a time: the lock is held until the answer comes
    // or the wait ends.
    let sender = sender.lock().map_err(|_| Error::Worker)?;
    if STUCK.load(Ordering::SeqCst) {
        return Err(Error::Busy);
    }
    let (reply_tx, reply_rx) = channel();
    sender.send((job, reply_tx)).map_err(|_| Error::Worker)?;
    match reply_rx.recv_timeout(Duration::from_millis(limit.saturating_add(MARGIN_MS))) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            STUCK.store(true, Ordering::SeqCst);
            Err(Error::Timeout)
        }
        Err(RecvTimeoutError::Disconnected) => Err(Error::Worker),
    }
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
