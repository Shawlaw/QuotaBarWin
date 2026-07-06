use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter};

use crate::{
    config::{config_path_for_app, load_or_create_config},
    quota::build_app_snapshot_from_config_path,
};

const MIN_REFRESH_INTERVAL_SECONDS: u64 = 10;
const INITIAL_REFRESH_DELAY: Duration = Duration::from_millis(750);
pub const SNAPSHOT_REFRESHED_EVENT: &str = "snapshot-refreshed";

static STARTED: AtomicBool = AtomicBool::new(false);
static WAKE_STATE: OnceLock<Arc<SchedulerWakeState>> = OnceLock::new();

struct SchedulerWakeState {
    generation: Mutex<u64>,
    wake: Condvar,
}

fn wake_state() -> Arc<SchedulerWakeState> {
    WAKE_STATE
        .get_or_init(|| {
            Arc::new(SchedulerWakeState {
                generation: Mutex::new(0),
                wake: Condvar::new(),
            })
        })
        .clone()
}

fn refresh_interval_delay(seconds: u64) -> Duration {
    Duration::from_secs(seconds.max(MIN_REFRESH_INTERVAL_SECONDS))
}

pub fn signal_config_changed() {
    if let Some(state) = WAKE_STATE.get() {
        if let Ok(mut generation) = state.generation.lock() {
            *generation = generation.wrapping_add(1);
            state.wake.notify_one();
        }
    }
}

pub fn start(app: AppHandle) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let state = wake_state();
    thread::Builder::new()
        .name("quotabarwin-refresh-scheduler".to_string())
        .spawn(move || run_scheduler(app, state))
        .expect("failed to start refresh scheduler");
}

fn run_scheduler(app: AppHandle, state: Arc<SchedulerWakeState>) {
    let mut observed_generation = current_generation(&state);
    observed_generation = wait_for_next_tick(&state, INITIAL_REFRESH_DELAY, observed_generation);
    loop {
        if let Err(error) = refresh_once(&app) {
            eprintln!("Background refresh failed: {error}");
        }

        let delay = refresh_delay_for_app(&app).unwrap_or_else(|error| {
            eprintln!("Unable to load refresh interval: {error}");
            refresh_interval_delay(MIN_REFRESH_INTERVAL_SECONDS)
        });

        observed_generation = wait_for_next_tick(&state, delay, observed_generation);
    }
}

fn refresh_once(app: &AppHandle) -> Result<(), String> {
    let path = config_path_for_app(app)?;
    let snapshot = build_app_snapshot_from_config_path(&path)?;
    app.emit(SNAPSHOT_REFRESHED_EVENT, snapshot)
        .map_err(|error| error.to_string())
}

fn refresh_delay_for_app(app: &AppHandle) -> Result<Duration, String> {
    let path = config_path_for_app(app)?;
    let config = load_or_create_config(&path)?.config;
    Ok(refresh_interval_delay(config.refresh_interval_seconds))
}

fn current_generation(state: &SchedulerWakeState) -> u64 {
    state
        .generation
        .lock()
        .map(|generation| *generation)
        .unwrap_or(0)
}

fn wait_for_next_tick(
    state: &SchedulerWakeState,
    delay: Duration,
    observed_generation: u64,
) -> u64 {
    let Ok(generation) = state.generation.lock() else {
        thread::sleep(delay);
        return observed_generation;
    };

    if *generation != observed_generation {
        return *generation;
    }

    state
        .wake
        .wait_timeout_while(generation, delay, |generation| {
            *generation == observed_generation
        })
        .map(|(generation, _)| *generation)
        .unwrap_or(observed_generation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_interval_delay_clamps_to_minimum() {
        assert_eq!(refresh_interval_delay(0), Duration::from_secs(10));
        assert_eq!(refresh_interval_delay(5), Duration::from_secs(10));
        assert_eq!(refresh_interval_delay(120), Duration::from_secs(120));
    }
}
