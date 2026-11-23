//! One thread owns the Giac library and its context.
//!
//! Giac's context is not safe to share between threads, and the reference
//! serialises its calls too (`callEvaluateFunction` is synchronized). Giac
//! also recurses deeply, so the thread gets a large stack instead of the
//! default one of a test thread.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

use crate::native::Native;
use crate::{session, Error, LIBRARY_VARIABLE};

const STACK_BYTES: usize = 256 << 20;

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
            }
        })
        .map_err(|error| Error::Load(error.to_string()))?;
    ready_rx.recv().map_err(|_| Error::Worker)??;
    Ok(Mutex::new(tx))
}

fn run(job: Job) -> Result<String, Error> {
    let sender = WORKER.get_or_init(start).as_ref().map_err(Clone::clone)?;
    let (reply_tx, reply_rx) = channel();
    sender
        .lock()
        .map_err(|_| Error::Worker)?
        .send((job, reply_tx))
        .map_err(|_| Error::Worker)?;
    reply_rx.recv().map_err(|_| Error::Worker)?
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
