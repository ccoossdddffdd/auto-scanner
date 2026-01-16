use chrono::{DateTime, Local};
use std::sync::{Arc, Mutex};

pub trait TimeProvider: Send + Sync {
    fn now(&self) -> DateTime<Local>;
}

pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}

pub struct MockTimeProvider {
    current_time: Arc<Mutex<DateTime<Local>>>,
}

impl MockTimeProvider {
    pub fn new(time: DateTime<Local>) -> Self {
        Self {
            current_time: Arc::new(Mutex::new(time)),
        }
    }

    pub fn set_time(&self, time: DateTime<Local>) {
        let mut t = self.current_time.lock().unwrap();
        *t = time;
    }
}

impl TimeProvider for MockTimeProvider {
    fn now(&self) -> DateTime<Local> {
        *self.current_time.lock().unwrap()
    }
}
