//! Pomodoro and stretch-reminder timers. Pure time logic; the caller polls
//! `poll()` each frame and reacts to the returned events.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Not running a pomodoro.
    Off,
    /// Focus interval.
    Work,
    /// Short break.
    Break,
}

#[derive(Clone, Copy, Debug)]
pub enum TimerEvent {
    /// Time to stretch (fires on an independent cadence from pomodoro).
    Stretch,
    /// Pomodoro switched into a work interval.
    WorkStarted,
    /// Pomodoro switched into a break interval.
    BreakStarted,
}

pub struct Timers {
    // --- Pomodoro ---
    pub phase: Phase,
    phase_ends: Instant,
    work_len: Duration,
    break_len: Duration,

    // --- Stretch reminder ---
    stretch_every: Duration,
    next_stretch: Instant,
}

impl Timers {
    pub fn new(now: Instant) -> Self {
        let stretch_every = Duration::from_secs(30 * 60); // every 30 min
        Self {
            phase: Phase::Off,
            phase_ends: now,
            work_len: Duration::from_secs(25 * 60),
            break_len: Duration::from_secs(5 * 60),
            stretch_every,
            next_stretch: now + stretch_every,
        }
    }

    pub fn pomodoro_active(&self) -> bool {
        self.phase != Phase::Off
    }

    /// Start (or restart) a pomodoro at the work phase.
    pub fn start_pomodoro(&mut self, now: Instant) {
        self.phase = Phase::Work;
        self.phase_ends = now + self.work_len;
    }

    /// Stop the pomodoro entirely.
    pub fn stop_pomodoro(&mut self) {
        self.phase = Phase::Off;
    }

    /// Remaining time in the current pomodoro phase, or None when off.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        if self.phase == Phase::Off {
            return None;
        }
        Some(self.phase_ends.saturating_duration_since(now))
    }

    /// "25:00" style label for the current phase, or None when off.
    pub fn label(&self, now: Instant) -> Option<String> {
        self.remaining(now).map(|r| {
            let secs = r.as_secs();
            format!("{:02}:{:02}", secs / 60, secs % 60)
        })
    }

    /// Advance timers; returns any events that fired this tick.
    pub fn poll(&mut self, now: Instant) -> Vec<TimerEvent> {
        let mut events = Vec::new();

        // Stretch reminder is independent of pomodoro.
        if now >= self.next_stretch {
            events.push(TimerEvent::Stretch);
            self.next_stretch = now + self.stretch_every;
        }

        // Pomodoro phase transitions.
        if self.phase != Phase::Off && now >= self.phase_ends {
            match self.phase {
                Phase::Work => {
                    self.phase = Phase::Break;
                    self.phase_ends = now + self.break_len;
                    events.push(TimerEvent::BreakStarted);
                }
                Phase::Break => {
                    self.phase = Phase::Work;
                    self.phase_ends = now + self.work_len;
                    events.push(TimerEvent::WorkStarted);
                }
                Phase::Off => {}
            }
        }

        events
    }
}
