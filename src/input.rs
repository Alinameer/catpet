//! Global input listener. Runs `rdev::listen` on a dedicated thread and forwards
//! coarse, debounced events to the main loop over a channel.
//!
//! We deliberately send *summaries* (typed / scrolled / moved-to X,Y / clicked)
//! rather than every raw event, so the UI thread never blocks on input volume.

use std::sync::mpsc::Sender;
use std::thread;

#[derive(Clone, Copy, Debug)]
pub enum InputEvent {
    /// Absolute cursor position in screen pixels.
    MouseMove { x: f64, y: f64 },
    /// A key was pressed (any key). Carries nothing but the fact of a keystroke.
    KeyPress,
    /// Mouse wheel moved.
    Scroll,
    /// A mouse button went down.
    Click,
}

/// Spawn the global listener thread. Returns immediately.
///
/// On X11 this requires the XTEST / XInput extensions (provided by libxi/libxtst).
/// If `rdev::listen` fails (e.g. under a pure-Wayland session with no XWayland
/// grab), the error is printed once and the thread exits; the pet still runs and
/// stays fully functional for everything except global-input reactions.
pub fn spawn(tx: Sender<InputEvent>) {
    thread::Builder::new()
        .name("pixelpal-input".into())
        .spawn(move || {
            let callback = move |event: rdev::Event| {
                let msg = match event.event_type {
                    rdev::EventType::MouseMove { x, y } => Some(InputEvent::MouseMove { x, y }),
                    rdev::EventType::KeyPress(_) => Some(InputEvent::KeyPress),
                    rdev::EventType::Wheel { .. } => Some(InputEvent::Scroll),
                    rdev::EventType::ButtonPress(_) => Some(InputEvent::Click),
                    _ => None,
                };
                if let Some(m) = msg {
                    // If the receiver is gone the app is shutting down; ignore.
                    let _ = tx.send(m);
                }
            };

            if let Err(err) = rdev::listen(callback) {
                eprintln!(
                    "[pixelpal] global input listener unavailable ({err:?}); \
                     mouse/keyboard reactions disabled. The pet still runs."
                );
            }
        })
        .expect("failed to spawn input thread");
}
