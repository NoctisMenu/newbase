use std::time::{Duration, Instant};

pub struct Spring {
    pub mass: f32,
    pub damping: f32,
    pub stiffness: f32,
    pub initial_velocity: f32,
}

pub struct Animation {
    pub linear_progress: f32,
    pub animation_start: Option<Instant>,
    s_value: f32,
    e_value: f32,
    animation_duration: Duration,
    pub progress: f32,
    pub spring: Option<Spring>,
}

impl Default for Animation {
    fn default() -> Self {
        Self::new(Duration::from_millis(700), Some(DEFAULT_SPRING))
    }
}

pub const DEFAULT_SPRING: Spring = Spring {
    mass: 1.0,
    damping: 12.0,
    stiffness: 200.0,
    initial_velocity: 10.0,
};

impl Animation {
    pub fn new(animation_duration: Duration, spring: Option<Spring>) -> Self {
        Self {
            linear_progress: 0.0,
            progress: 0.0,
            e_value: 1.0,
            s_value: 0.0,
            animation_start: Some(std::time::Instant::now()),
            animation_duration,
            spring: spring,
        }
    }
    pub fn set_spring(mut self, spring: Spring) -> Self {
        self.spring = Some(spring);
        self
    }
    pub fn set_duration(mut self, duration: Duration) -> Self {
        self.animation_duration = duration;
        self
    }
    pub fn set_values(&mut self, values: (f32, f32)) {
        self.s_value = values.1;
        self.e_value = values.0;
    }
    pub fn start(&mut self, start: Instant) {
        self.animation_start = Some(start);
    }
    pub fn update(&mut self) {
        if let Some(start) = self.animation_start {
            let elapsed = start.elapsed();
            self.linear_progress =
                (elapsed.as_secs_f32() / self.animation_duration.as_secs_f32()).clamp(0.0, 1.0);
            //map it so that 0.0 -> s_value and 1.0 -> e_value
            self.linear_progress =
                self.s_value + (self.e_value - self.s_value) * self.linear_progress;

            if let Some(spring) = &self.spring {
                // spring physics formula
                let t = elapsed.as_secs_f32();
                let m = spring.mass;
                let k = spring.stiffness;
                let c = spring.damping;
                let v0 = spring.initial_velocity;

                let zeta = c / (2.0 * (k * m).sqrt()); // damping ratio
                let omega0 = (k / m).sqrt(); // undamped angular frequency
                let omega1 = omega0 * (1.0 - zeta * zeta).sqrt(); // damped angular frequency

                if zeta < 1.0 {
                    // underdamped
                    self.progress = 1.0
                        - ((-(zeta * omega0 * t)).exp()
                            * ((v0 + zeta * omega0) / omega1 * (omega1 * t).sin()
                                + (omega1 * t).cos()));
                } else if zeta == 1.0 {
                    // critically damped
                    self.progress = 1.0 - (-(omega0 * t)).exp() * (1.0 + (v0 + omega0) * t);
                } else {
                    // overdamped
                    let r1 = -omega0 * (zeta - (zeta * zeta - 1.0).sqrt());
                    let r2 = -omega0 * (zeta + (zeta * zeta - 1.0).sqrt());
                    self.progress = 1.0
                        - ((v0 - r2) / (r1 - r2) * (r1).exp() + (r1 - v0) / (r1 - r2) * (r2).exp());
                }
                self.progress = self.s_value + self.progress * (self.e_value - self.s_value);
            } else {
                self.progress = self.linear_progress;
            }
            if elapsed >= self.animation_duration {
                self.animation_start = None; // stop animation
                self.linear_progress = self.e_value;
                self.progress = self.e_value;
            }
        } else {
            self.linear_progress = self.e_value
        }
    }
}
