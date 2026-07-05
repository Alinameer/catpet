//! The cat's behavioral state: mood, animation phase, and how it decays over time.

use std::time::{Duration, Instant};

/// High-level things the cat can be doing. Higher-priority moods win when several
/// could apply at once (see `Mood::priority`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    /// Default: idle, breathing, occasional blink, eyes track the cursor.
    Idle,
    /// Cursor is right over the cat and (optionally) being pressed: purring / happy.
    Petted,
    /// User is typing: paws knead the "keyboard".
    Typing,
    /// User is scrolling: head/ears twitch.
    Scrolling,
    /// Stretch reminder fired: cat does a big stretch and shows a bubble.
    Stretch,
    /// Pomodoro state changed (work<->break): cat reacts + bubble.
    Pomodoro,
    /// Claude Code finished: the cat jumps and meows at you.
    WorkDone,
}

impl Mood {
    /// When multiple moods are eligible, the highest priority is shown.
    fn priority(self) -> u8 {
        match self {
            Mood::WorkDone => 100,
            Mood::Pomodoro => 90,
            Mood::Stretch => 80,
            Mood::Petted => 60,
            Mood::Typing => 40,
            Mood::Scrolling => 30,
            Mood::Idle => 0,
        }
    }
}

/// A short-lived overlay message shown above the cat (e.g. "stretch!", "focus 25:00").
#[derive(Clone, Debug)]
pub struct Bubble {
    pub text: String,
    pub until: Instant,
}

pub struct CatState {
    /// The mood currently being displayed.
    pub mood: Mood,
    /// When the current (transient) mood should expire back toward Idle.
    mood_until: Instant,
    /// Continuous animation clock in seconds since start; drives breathing/blink/tail.
    pub clock: f32,
    started: Instant,
    /// Normalized cursor position relative to the cat window centre, in [-1, 1].
    /// Used so the eyes can look toward the cursor.
    pub look: (f32, f32),
    /// Blink phase: 0.0 = eyes open, 1.0 = fully shut. Driven internally.
    pub blink: f32,
    next_blink: Instant,
    /// A transient bubble, if any.
    pub bubble: Option<Bubble>,
    /// Rises toward 1.0 while typing fast, decays otherwise; scales knead speed.
    pub energy: f32,
    /// One-shot vertical hop offset (pixels) for the WorkDone jump, decays to 0.
    pub hop: f32,
    /// Mochi squash/stretch: >0 stretches vertically (lifted/dragged), <0 squashes.
    /// Settles back to 0 with a springy wobble.
    pub squash: f32,
    /// Velocity term for the squash spring.
    squash_v: f32,
    /// True while the cat is being dragged by the mouse.
    pub dragging: bool,
    /// Horizontal wobble (radians-ish) while shaking during a drag.
    pub wobble: f32,
}

impl CatState {
    pub fn new(now: Instant) -> Self {
        Self {
            mood: Mood::Idle,
            mood_until: now,
            clock: 0.0,
            started: now,
            look: (0.0, 0.0),
            blink: 0.0,
            next_blink: now + Duration::from_secs(3),
            bubble: None,
            energy: 0.0,
            hop: 0.0,
            squash: 0.0,
            squash_v: 0.0,
            dragging: false,
            wobble: 0.0,
        }
    }

    /// Request a transient mood lasting `dur`. Ignored if a higher-priority mood
    /// is currently active and still valid.
    pub fn set_mood(&mut self, mood: Mood, dur: Duration, now: Instant) {
        let current_valid = now < self.mood_until;
        if current_valid && self.mood.priority() > mood.priority() {
            return;
        }
        self.mood = mood;
        self.mood_until = now + dur;
    }

    pub fn show_bubble(&mut self, text: impl Into<String>, dur: Duration, now: Instant) {
        self.bubble = Some(Bubble {
            text: text.into(),
            until: now + dur,
        });
    }

    pub fn set_look(&mut self, nx: f32, ny: f32) {
        // Clamp and damp so the eyes don't jump to the extremes.
        self.look = (nx.clamp(-1.0, 1.0) * 0.7, ny.clamp(-1.0, 1.0) * 0.7);
    }

    pub fn bump_energy(&mut self, amount: f32) {
        self.energy = (self.energy + amount).min(1.0);
    }

    pub fn trigger_hop(&mut self) {
        self.hop = 22.0;
    }

    /// Called when the drag begins: lift the cat (stretch like mochi).
    pub fn start_drag(&mut self) {
        self.dragging = true;
        self.squash = 0.35; // stretched tall
        self.squash_v = 0.0;
    }

    /// Called when released: let it spring back with a squash-and-settle.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.squash = -0.25; // land squashed, spring resolves it
        self.squash_v = 0.0;
        self.wobble = 0.0;
    }

    /// Feed drag movement so a fast shake makes it wobble side to side.
    pub fn drag_move(&mut self, speed: f32) {
        // speed is pixels/frame magnitude; map to a wobble impulse.
        self.wobble = (self.wobble + speed * 0.02).clamp(0.0, 1.2);
    }

    /// Advance all time-based animation. `dt` is seconds since the last frame.
    pub fn tick(&mut self, now: Instant, dt: f32) {
        self.clock = now.duration_since(self.started).as_secs_f32();

        // Expire transient moods.
        if now >= self.mood_until && self.mood != Mood::Idle {
            self.mood = Mood::Idle;
        }
        // Expire bubble.
        if let Some(b) = &self.bubble {
            if now >= b.until {
                self.bubble = None;
            }
        }

        // Blink scheduling: quick close/open, then schedule the next blink.
        if self.blink > 0.0 {
            // Close fast, open a touch slower.
            self.blink -= dt * 6.0;
            if self.blink < 0.0 {
                self.blink = 0.0;
            }
        } else if now >= self.next_blink {
            self.blink = 1.0;
            // Deterministic-ish varied cadence without needing an RNG crate.
            let jitter = ((self.clock * 1.37).sin() * 1.5 + 3.5).abs();
            self.next_blink = now + Duration::from_secs_f32(jitter);
        }

        // Energy decays over time.
        self.energy = (self.energy - dt * 0.6).max(0.0);

        // Hop decays (ease out).
        if self.hop > 0.0 {
            self.hop -= dt * 90.0;
            if self.hop < 0.0 {
                self.hop = 0.0;
            }
        }

        // Mochi squash spring. While dragging, hold the stretched pose; once
        // released, a damped spring pulls `squash` back to 0 with a wobble.
        if self.dragging {
            self.squash += (0.35 - self.squash) * (dt * 10.0).min(1.0);
        } else {
            let k = 90.0; // stiffness
            let d = 9.0; // damping
            let accel = -k * self.squash - d * self.squash_v;
            self.squash_v += accel * dt;
            self.squash += self.squash_v * dt;
            if self.squash.abs() < 0.003 && self.squash_v.abs() < 0.02 {
                self.squash = 0.0;
                self.squash_v = 0.0;
            }
        }

        // Wobble decays quickly.
        self.wobble = (self.wobble - dt * 3.0).max(0.0);
    }
}
