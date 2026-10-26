//! Parallel evaluation of a batch of points.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Number of threads for a `threads` option: 0 means all cores.
pub(crate) fn thread_count(threads: usize) -> usize {
    if threads > 0 {
        threads
    } else {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    }
}

/// Applies `f` to every item on up to `threads` threads (0: all cores).
/// The results are in the order of the items, whatever the scheduling.
pub(crate) fn map<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: &(impl Fn(&T) -> R + Sync),
) -> Vec<R> {
    let threads = thread_count(threads).min(items.len());
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        out.push((i, f(item)));
                    }
                    out
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_kept() {
        let items: Vec<u64> = (0..1000).collect();
        let slow_square = |&i: &u64| {
            // Uneven work, so that the threads finish out of order.
            (0..(i % 7) * 1000).fold(i * i, |a, _| std::hint::black_box(a))
        };
        let expected: Vec<u64> = items.iter().map(|i| i * i).collect();
        for threads in [0, 1, 3, 16] {
            assert_eq!(map(&items, threads, &slow_square), expected);
        }
        assert!(map(&[] as &[u64], 4, &slow_square).is_empty());
    }
}
