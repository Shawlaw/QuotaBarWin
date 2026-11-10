//! In-process per-provider read/write lock.
//!
//! Wired in CP0b-2 (cache publish, adopt, test snapshot). Rules documented for
//! the migration:
//!
//! 1. Lock order is **portable → provider → config**. Never acquire a provider
//!    lock while holding the config write lock.
//! 2. Never perform network I/O inside a provider lock (download/test/update
//!    checks happen outside; the lock only covers local cache/config work).
//! 3. Do not nest locks for the same provider id (std `RwLock` is not
//!    re-entrant); different ids are independent.
//! 4. The registry mutex is only held while looking up/inserting a lock; it is
//!    never held while the provider lock is held.
#![allow(dead_code)] // wired in CP0b-2

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<RwLock<()>>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Arc<RwLock<()>>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_for(provider_id: &str) -> Arc<RwLock<()>> {
    let mut locks = registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks
        .entry(provider_id.to_string())
        .or_insert_with(|| Arc::new(RwLock::new(())))
        .clone()
}

/// Runs `action` while holding the shared (reader) lock for `provider_id`.
///
/// Readers are the manifest/source snapshot, execution fingerprint and runner
/// snapshot paths; publishers take the write lock.
pub fn with_provider_read<T>(provider_id: &str, action: impl FnOnce() -> T) -> T {
    let lock = lock_for(provider_id);
    let _guard = lock.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    action()
}

/// Runs `action` while holding the exclusive (writer) lock for `provider_id`.
///
/// Used for cache generation publish, provider removal and adopt.
pub fn with_provider_write<T>(provider_id: &str, action: impl FnOnce() -> T) -> T {
    let lock = lock_for(provider_id);
    let _guard = lock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    action()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    // Each test uses a distinct provider id: the registry is process-global and
    // cargo runs tests in parallel.

    #[test]
    fn writes_to_the_same_provider_are_serialized() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            handles.push(thread::spawn(move || {
                with_provider_write("kimi-serialized", || {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(current, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(2));
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }));
        }
        for handle in handles {
            handle.join().expect("writer thread");
        }

        assert_eq!(
            max_active.load(Ordering::SeqCst),
            1,
            "only one writer may hold the same provider lock"
        );
    }

    #[test]
    fn different_providers_hold_write_locks_concurrently() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let first = thread::spawn(move || {
            with_provider_write("kimi-different", || {
                started_tx.send(()).expect("signal start");
                release_rx.recv().expect("wait for release");
            });
        });
        started_rx.recv().expect("first writer acquired");

        let (second_tx, second_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            with_provider_write("other-different", || {
                second_tx.send(()).expect("signal second");
            });
        });

        assert!(
            second_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "a different provider must not be blocked"
        );

        release_tx.send(()).expect("release first");
        first.join().expect("first thread");
        second.join().expect("second thread");
    }

    #[test]
    fn readers_of_the_same_provider_run_concurrently() {
        let (first_tx, first_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let first = thread::spawn(move || {
            with_provider_read("kimi-readers", || {
                first_tx.send(()).expect("signal first reader");
                release_rx.recv().expect("wait for release");
            });
        });
        first_rx.recv().expect("first reader acquired");

        let (second_tx, second_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            with_provider_read("kimi-readers", || {
                second_tx.send(()).expect("signal second reader");
            });
        });

        assert!(
            second_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "readers must share the lock"
        );

        release_tx.send(()).expect("release first");
        first.join().expect("first thread");
        second.join().expect("second thread");
    }

    #[test]
    fn a_writer_blocks_readers_until_it_finishes() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let writer = thread::spawn(move || {
            with_provider_write("kimi-blocked", || {
                started_tx.send(()).expect("signal writer");
                release_rx.recv().expect("wait for release");
            });
        });
        started_rx.recv().expect("writer acquired");

        let (reader_tx, reader_rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            with_provider_read("kimi-blocked", || {
                reader_tx.send(()).expect("signal reader");
            });
        });

        assert!(
            reader_rx.recv_timeout(Duration::from_millis(150)).is_err(),
            "reader must wait for the writer"
        );

        release_tx.send(()).expect("release writer");
        reader_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("reader acquired after release");

        writer.join().expect("writer thread");
        reader.join().expect("reader thread");
    }
}
