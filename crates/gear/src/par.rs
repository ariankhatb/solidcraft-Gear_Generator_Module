//! Work split over the machine's cores (scoped threads, no dependencies): the mesh checks' probes
//! and surfaces are independent of each other, so the window's verdict comes back several times
//! sooner. Results keep the inputs' order, so whatever is folded from them is the same as one
//! thread would make.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Threads to use for `n` jobs: none spare for fewer than two jobs, at most the cores (and 8).
fn threads_for(n: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |c| c.get());
    cores.min(8).min(n).max(1)
}

/// `f` of each of `0..n`, in order, computed on several threads (jobs handed out one at a time,
/// so uneven jobs balance).
pub fn map<R: Send>(n: usize, f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    let threads = threads_for(n);
    if threads <= 1 {
        return (0..n).map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut out: Vec<Option<R>> = (0..n).map(|_| None).collect();
    let done: Vec<Vec<(usize, R)>> = std::thread::scope(|sc| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                sc.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= n {
                            break;
                        }
                        mine.push((i, f(i)));
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    });
    for (i, r) in done.into_iter().flatten() {
        out[i] = Some(r);
    }
    out.into_iter().map(|r| r.expect("every job ran")).collect()
}

/// `a()` and `b()` at once.
pub fn join<A: Send, B: Send>(a: impl FnOnce() -> A + Send, b: impl FnOnce() -> B + Send) -> (A, B) {
    if threads_for(2) < 2 {
        return (a(), b());
    }
    std::thread::scope(|sc| {
        let hb = sc.spawn(b);
        let ra = a();
        (ra, hb.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
    })
}
