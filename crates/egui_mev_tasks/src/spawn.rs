/// Run `job` in the background where possible.
///
/// Desktop builds run it on its own thread, which ends when the job returns:
/// nothing can stop it sooner, so what it captures lives until then. The web
/// build has no threads (that needs cross-origin isolation, which static hosts
/// like GitHub Pages can't enable), so the job runs to completion inline and
/// its results are waiting on their channel by the next frame.
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(job);
    #[cfg(target_arch = "wasm32")]
    job();
}

#[cfg(test)]
mod tests {
    use crate::test_support::TIMEOUT;

    #[test]
    fn runs_the_job() {
        let (tx, rx) = std::sync::mpsc::channel();
        super::spawn(move || tx.send(7).expect("send"));
        assert_eq!(rx.recv_timeout(TIMEOUT), Ok(7));
    }
}
