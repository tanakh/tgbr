use std::{
    collections::VecDeque,
    time::{Duration, SystemTime},
};

pub struct Timer {
    hist: VecDeque<SystemTime>,
    prev: SystemTime,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            hist: VecDeque::new(),
            prev: SystemTime::now(),
        }
    }

    pub fn wait_for_frame(&mut self, fps: f64) {
        let span = 1.0 / fps;

        let elapsed = self.prev.elapsed().unwrap().as_secs_f64();

        if elapsed < span {
            let wait = span - elapsed;
            std::thread::sleep(Duration::from_secs_f64(wait));
        }

        self.prev = SystemTime::now();

        self.hist.push_back(self.prev);
        while self.hist.len() > 60 {
            self.hist.pop_front();
        }
    }

    pub fn fps(&self) -> f64 {
        if self.hist.len() < 60 {
            return 0.0;
        }

        let span = self.hist.len() - 1;
        let dur = self
            .hist
            .back()
            .unwrap()
            .duration_since(*self.hist.front().unwrap())
            .unwrap()
            .as_secs_f64();

        span as f64 / dur
    }
}
