//! A bounded pool producing one value per key off the UI thread. Handing the
//! results to egui (texture upload and so on) remains on the UI thread.

use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use egui::Context;

type Work<K, V> = dyn Fn(&K) -> V + Send + Sync;

struct Job<K> {
    key: K,
    epoch: u64,
    /// Carried by the job so that idle threads hold no context.
    ctx: Context,
}

struct Finished<K, V> {
    key: K,
    epoch: u64,
    value: V,
}

struct Queue<K> {
    jobs: VecDeque<Job<K>>,
    closed: bool,
}

struct Shared<K> {
    queue: Mutex<Queue<K>>,
    ready: Condvar,
}

impl<K> Shared<K> {
    fn lock(&self) -> MutexGuard<'_, Queue<K>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Block until there's a job. `None` means the worker was dropped.
    fn next_job(&self) -> Option<Job<K>> {
        let mut queue = self.lock();
        loop {
            if queue.closed {
                return None;
            }
            if let Some(job) = queue.jobs.pop_front() {
                return Some(job);
            }
            queue = self
                .ready
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Runs `work` for submitted keys on background threads and hands back the
/// finished `(key, value)` pairs, waking egui as each one is ready.
///
/// A key is in flight from [`submit`](Self::submit) until
/// [`drain`](Self::drain) returns it. Submitting a key already in flight does
/// nothing, and neither does submitting past the queue bound, so calling
/// `submit` every frame for whatever is on screen is fine: a refused key gets
/// in on a later frame.
///
/// [`invalidate`](Self::invalidate) is for when the inputs to `work` change: no
/// result of work requested before it will surface.
///
/// Threads start on the first submit. Dropping the worker ends them: a running
/// job is left to finish without blocking the UI, and queued jobs never run.
///
/// With no threads, on the web or if `threads` is 0 or the OS refuses to start
/// any, `submit` runs the work inline and its result waits for the next
/// `drain`. Everything else behaves the same.
///
/// If `work` panics, the thread survives and that key stays in flight, so it
/// isn't retried until the next `invalidate`. A failure the app wants to see or
/// retry belongs in `V`.
pub struct KeyedWorker<K, V> {
    work: Arc<Work<K, V>>,
    threads: usize,
    queue_bound: usize,
    /// How many threads started; `None` before the first submit.
    workers: Option<usize>,
    shared: Arc<Shared<K>>,
    results_tx: Sender<Finished<K, V>>,
    results_rx: Receiver<Finished<K, V>>,
    pending: HashSet<K>,
    epoch: u64,
}

impl<K, V> KeyedWorker<K, V>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Send + 'static,
{
    /// A worker with `threads` threads that keeps at most `queue_bound` keys in
    /// flight: queued, running, or finished and waiting for `drain`.
    #[must_use]
    pub fn new(
        threads: usize,
        queue_bound: usize,
        work: impl Fn(&K) -> V + Send + Sync + 'static,
    ) -> Self {
        let (results_tx, results_rx) = mpsc::channel();
        Self {
            work: Arc::new(work),
            threads,
            queue_bound,
            workers: None,
            shared: Arc::new(Shared {
                queue: Mutex::new(Queue {
                    jobs: VecDeque::new(),
                    closed: false,
                }),
                ready: Condvar::new(),
            }),
            results_tx,
            results_rx,
            pending: HashSet::new(),
            epoch: 0,
        }
    }

    /// Ask for `key`'s value, waking `ctx` when it's ready. Returns whether the
    /// key was accepted: `false` means it's in flight already or the worker is
    /// at its bound.
    pub fn submit(&mut self, key: K, ctx: &Context) -> bool {
        if self.pending.contains(&key) || self.pending.len() >= self.queue_bound {
            return false;
        }
        let workers = match self.workers {
            Some(workers) => workers,
            None => *self.workers.insert(self.start_threads()),
        };
        self.pending.insert(key.clone());
        let job = Job {
            key,
            epoch: self.epoch,
            ctx: ctx.clone(),
        };
        if workers == 0 {
            run_job(job, self.work.as_ref(), &self.results_tx);
        } else {
            self.shared.lock().jobs.push_back(job);
            self.shared.ready.notify_one();
        }
        true
    }

    /// The results finished since the last call. With more than one thread they
    /// come in the order they finished.
    pub fn drain(&mut self) -> Vec<(K, V)> {
        let mut values = Vec::new();
        for result in self.results_rx.try_iter() {
            if result.epoch != self.epoch {
                continue;
            }
            self.pending.remove(&result.key);
            values.push((result.key, result.value));
        }
        values
    }

    /// Forget everything in flight: queued jobs are dropped, and the result of
    /// a running job is thrown away when it arrives. The same keys can be
    /// submitted again straight away.
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.pending.clear();
        self.shared.lock().jobs.clear();
        // Stale results would only be skipped by the next drain, and an app
        // that invalidates without draining would pile them up.
        for _ in self.results_rx.try_iter() {}
    }

    /// Whether `key` is queued, running, or finished and waiting for `drain`.
    #[must_use]
    pub fn is_pending(&self, key: &K) -> bool {
        self.pending.contains(key)
    }

    /// How many keys are in flight.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Start the threads, returning how many did. None on the web, where
    /// waiting on the queue isn't possible anyway.
    fn start_threads(&self) -> usize {
        let threads = if cfg!(target_arch = "wasm32") {
            0
        } else {
            self.threads
        };
        let mut started = 0;
        for index in 0..threads {
            let shared = Arc::clone(&self.shared);
            let work = Arc::clone(&self.work);
            let results = self.results_tx.clone();
            let spawned = thread::Builder::new()
                .name(format!("keyed-worker-{index}"))
                .spawn(move || {
                    while let Some(job) = shared.next_job() {
                        run_job(job, work.as_ref(), &results);
                    }
                });
            if spawned.is_ok() {
                started += 1;
            }
        }
        started
    }
}

fn run_job<K, V>(job: Job<K>, work: &Work<K, V>, results: &Sender<Finished<K, V>>) {
    let Job { key, epoch, ctx } = job;
    // Without this a panic would quietly lose a thread, and the last one lost
    // would leave every later key waiting forever.
    let Ok(value) = catch_unwind(AssertUnwindSafe(|| work(&key))) else {
        return;
    };
    // Send first, so the frame this wake causes finds the result.
    if results.send(Finished { key, epoch, value }).is_ok() {
        ctx.request_repaint();
    }
}

impl<K, V> Drop for KeyedWorker<K, V> {
    fn drop(&mut self) {
        let mut queue = self.shared.lock();
        queue.closed = true;
        queue.jobs.clear();
        drop(queue);
        self.shared.ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    };

    use super::{Finished, KeyedWorker};
    use crate::test_support::{TIMEOUT, repaint_signal};

    /// The value is the key and which run of the work produced it, from 1.
    type Worker = KeyedWorker<String, (String, usize)>;

    /// Controls a [`gated`] worker. Dropping `release` lets all work through.
    struct Gate {
        started: Receiver<String>,
        release: Sender<()>,
    }

    impl Gate {
        fn wait_started(&self) -> String {
            self.started.recv_timeout(TIMEOUT).expect("work started")
        }

        fn release_one(&self) {
            self.release.send(()).expect("work is waiting");
        }
    }

    /// A worker whose work reports that it started, then waits for the gate.
    fn gated(threads: usize, queue_bound: usize) -> (Worker, Gate) {
        let (started_tx, started) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let runs = AtomicUsize::new(0);
        let worker = KeyedWorker::new(threads, queue_bound, move |key: &String| {
            let run = runs.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = started_tx.send(key.clone());
            let _ = release_rx.lock().expect("gate lock").recv_timeout(TIMEOUT);
            (key.clone(), run)
        });
        (worker, Gate { started, release })
    }

    /// Block until `count` results have arrived, leaving them to be drained.
    fn wait_for_results(worker: &Worker, count: usize) {
        let results: Vec<_> = (0..count)
            .map(|_| worker.results_rx.recv_timeout(TIMEOUT).expect("result"))
            .collect();
        for result in results {
            worker.results_tx.send(result).expect("own channel");
        }
    }

    fn key(name: &str) -> String {
        name.to_owned()
    }

    #[test]
    fn stale_results_are_discarded_after_invalidation() {
        let mut worker: KeyedWorker<String, ()> = KeyedWorker::new(1, 8, |_| ());
        let finished = |key: &str, epoch| Finished {
            key: key.to_owned(),
            epoch,
            value: (),
        };
        worker.results_tx.send(finished("old", 0)).expect("result");
        worker.invalidate();
        assert!(worker.drain().is_empty());

        worker.results_tx.send(finished("late", 0)).expect("result");
        assert!(worker.drain().is_empty(), "arrived after the invalidate");

        worker.results_tx.send(finished("new", 1)).expect("result");
        assert_eq!(worker.drain(), vec![(key("new"), ())]);
    }

    #[test]
    fn work_from_before_an_invalidate_never_surfaces() {
        let ctx = egui::Context::default();
        let (mut worker, gate) = gated(1, 8);

        assert!(worker.submit(key("old"), &ctx));
        assert_eq!(gate.wait_started(), "old");
        assert!(worker.submit(key("queued"), &ctx));

        worker.invalidate();
        assert_eq!(worker.pending_len(), 0);
        assert!(worker.submit(key("old"), &ctx), "may be asked for again");

        gate.release_one();
        assert_eq!(gate.wait_started(), "old", "the queued job was dropped");
        gate.release_one();

        wait_for_results(&worker, 2);
        assert_eq!(worker.drain(), vec![(key("old"), (key("old"), 2))]);
        assert_eq!(worker.pending_len(), 0);
        assert!(gate.started.try_recv().is_err(), "nothing else ran");
    }

    #[test]
    fn keys_in_flight_are_deduplicated() {
        let ctx = egui::Context::default();
        let (mut worker, gate) = gated(1, 8);

        assert!(worker.submit(key("a"), &ctx));
        assert!(!worker.submit(key("a"), &ctx), "queued or running");
        assert_eq!(gate.wait_started(), "a");
        assert!(!worker.submit(key("a"), &ctx), "running");
        assert!(worker.is_pending(&key("a")));

        gate.release_one();
        wait_for_results(&worker, 1);
        assert!(!worker.submit(key("a"), &ctx), "finished but not drained");

        assert_eq!(worker.drain(), vec![(key("a"), (key("a"), 1))]);
        assert!(!worker.is_pending(&key("a")));
        assert!(worker.submit(key("a"), &ctx), "drained, so it's new again");
    }

    #[test]
    fn submissions_are_bounded() {
        let ctx = egui::Context::default();
        let (mut worker, gate) = gated(1, 32);

        let accepted = (0..100)
            .filter(|index: &i32| worker.submit(index.to_string(), &ctx))
            .count();
        assert_eq!(accepted, 32);
        assert_eq!(worker.pending_len(), 32);

        drop(gate.release);
        wait_for_results(&worker, 32);
        assert!(!worker.submit(key("extra"), &ctx), "not drained yet");
        assert_eq!(worker.drain().len(), 32);
        assert!(
            worker.submit(key("extra"), &ctx),
            "drained, so there's room"
        );
    }

    #[test]
    fn a_finished_result_wakes_the_context() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let mut worker = KeyedWorker::new(1, 8, |key: &i32| key * 2);
        worker.submit(21, &ctx);
        repainted.recv_timeout(TIMEOUT).expect("woken");
        assert_eq!(worker.drain(), vec![(21, 42)], "ready before the wake");
    }

    #[test]
    fn dropping_ends_a_busy_thread_and_skips_queued_jobs() {
        let ctx = egui::Context::default();
        let (mut worker, gate) = gated(1, 8);
        worker.submit(key("running"), &ctx);
        assert_eq!(gate.wait_started(), "running");
        worker.submit(key("queued"), &ctx);

        drop(worker);
        drop(gate.release);

        // The closure owns the sender, and the last thread to end drops it.
        assert_eq!(
            gate.started.recv_timeout(TIMEOUT),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn dropping_ends_idle_threads() {
        let ctx = egui::Context::default();
        let (mut worker, gate) = gated(3, 8);
        worker.submit(key("a"), &ctx);
        assert_eq!(gate.wait_started(), "a");
        gate.release_one();
        wait_for_results(&worker, 1);

        drop(worker);

        assert_eq!(
            gate.started.recv_timeout(TIMEOUT),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn dropping_with_undrained_results_does_not_hang() {
        let ctx = egui::Context::default();
        let (mut worker, gate) = gated(1, 8);
        worker.submit(key("a"), &ctx);
        worker.submit(key("b"), &ctx);
        assert_eq!(gate.wait_started(), "a");
        drop(worker);
        gate.release_one();
        assert_eq!(
            gate.started.recv_timeout(TIMEOUT),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    /// The path the web build takes.
    #[test]
    fn with_no_threads_work_runs_inline() {
        let ctx = egui::Context::default();
        let repainted = repaint_signal(&ctx);
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&runs);
        let mut worker = KeyedWorker::new(0, 2, move |key: &i32| {
            counted.fetch_add(1, Ordering::SeqCst);
            key * 2
        });

        assert!(worker.submit(1, &ctx));
        assert_eq!(runs.load(Ordering::SeqCst), 1, "ran during submit");
        repainted.try_recv().expect("woken");

        assert!(!worker.submit(1, &ctx), "not drained yet");
        assert!(worker.submit(2, &ctx));
        assert!(!worker.submit(3, &ctx), "at the bound");
        assert_eq!(runs.load(Ordering::SeqCst), 2);

        assert_eq!(worker.drain(), vec![(1, 2), (2, 4)]);

        assert!(worker.submit(1, &ctx));
        worker.invalidate();
        assert!(worker.drain().is_empty(), "stale");
    }

    #[test]
    fn a_panic_loses_the_key_but_not_the_thread() {
        let ctx = egui::Context::default();
        let mut worker = KeyedWorker::new(1, 8, |key: &i32| {
            assert_ne!(*key, 0, "this panic is the test");
            *key
        });
        let repainted = repaint_signal(&ctx);

        assert!(worker.submit(0, &ctx));
        assert!(worker.submit(1, &ctx));
        repainted
            .recv_timeout(TIMEOUT)
            .expect("second key finished");
        assert_eq!(worker.drain(), vec![(1, 1)]);

        assert!(!worker.submit(0, &ctx), "not retried");
        worker.invalidate();
        assert!(worker.submit(0, &ctx), "until the next invalidate");
    }
}
