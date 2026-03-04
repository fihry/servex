use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

trait Clock: Send + Sync {
    fn now_instant(&self) -> Instant;
    fn now_system(&self) -> SystemTime;
}

struct SystemClock;

impl Clock for SystemClock {
    fn now_instant(&self) -> Instant {
        Instant::now()
    }

    fn now_system(&self) -> SystemTime {
        SystemTime::now()
    }
}

struct ManualClock {
    base_instant: Instant,
    base_system: SystemTime,
    step: Duration,
    tick_file: PathBuf,
}

impl ManualClock {
    fn new(step: Duration, tick_file: PathBuf) -> Self {
        Self {
            base_instant: Instant::now(),
            base_system: SystemTime::now(),
            step,
            tick_file,
        }
    }

    fn ticks(&self) -> u64 {
        fs::read_to_string(&self.tick_file)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0)
    }
}

impl Clock for ManualClock {
    fn now_instant(&self) -> Instant {
        self.base_instant + self.step.saturating_mul(self.ticks() as u32)
    }

    fn now_system(&self) -> SystemTime {
        self.base_system + self.step.saturating_mul(self.ticks() as u32)
    }
}

static CLOCK: OnceLock<Box<dyn Clock>> = OnceLock::new();

fn provider() -> &'static dyn Clock {
    CLOCK.get_or_init(init_clock).as_ref()
}

fn init_clock() -> Box<dyn Clock> {
    if env::var("SERVEX_CLOCK_MODE")
        .map(|v| v.eq_ignore_ascii_case("manual"))
        .unwrap_or(false)
    {
        let step_ms = env::var("SERVEX_CLOCK_STEP_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(1000);
        let tick_file = env::var("SERVEX_CLOCK_TICK_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp/servex_clock_ticks"));
        return Box::new(ManualClock::new(Duration::from_millis(step_ms), tick_file));
    }
    Box::new(SystemClock)
}

pub(super) fn now_instant() -> Instant {
    provider().now_instant()
}

pub(super) fn unix_millis() -> u128 {
    provider()
        .now_system()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub(super) fn unix_nanos() -> u128 {
    provider()
        .now_system()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
