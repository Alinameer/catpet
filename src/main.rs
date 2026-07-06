//! catpet — a ComNyang-style desktop cat.
//!
//! Usage:
//!   catpet                 run the pet window
//!   catpet meow            tell a running pet: "work done", jump + meow
//!   catpet pomodoro        toggle the pomodoro timer
//!   catpet stretch         trigger a stretch reminder now
//!   catpet color <name>    set fur colour (orange grey black white brown blue pink)
//!   catpet pattern <name>  set pattern (solid tabby spots tuxedo)
//!   catpet quit            close a running pet
//!
//! The `meow`/`pomodoro`/... subcommands just connect to the running pet's Unix
//! socket (see ipc.rs). If no pet is running they print a hint and exit.

mod config;
mod font;
mod input;
mod ipc;
mod menu;
mod render;
mod sound;
mod sprite;
mod state;
mod timers;

use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use config::Config;
use input::InputEvent;
use ipc::Command as IpcCmd;
use state::{CatState, Mood};
use timers::{TimerEvent, Timers};

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId, WindowLevel};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // CLI subcommands talk to a running pet over the socket.
    if let Some(cmd) = args.first() {
        match cmd.as_str() {
            "meow" | "pomodoro" | "stretch" | "quit" => {
                send_or_hint(cmd);
                return;
            }
            "color" | "pattern" | "character" => {
                let arg = args.get(1).cloned().unwrap_or_default();
                send_or_hint(&format!("{cmd} {arg}"));
                return;
            }
            "name" => {
                // Name may contain spaces: join the rest.
                let arg = args[1..].join(" ");
                send_or_hint(&format!("name {arg}"));
                return;
            }
            "--help" | "-h" | "help" => {
                print_help();
                return;
            }
            "--dump" => {
                dump_frames(&args.get(1).cloned().unwrap_or_else(|| "/tmp".into()));
                return;
            }
            _ => {}
        }
    }

    ensure_assets();
    run_pet();
}

/// Dev-only: render sample frames to PNGs so the look can be reviewed without a
/// display. `catpet --dump <dir>`.
fn dump_frames(dir: &str) {
    use state::Mood;
    let sprites = sprite::Sprites::load();
    let now = Instant::now();
    let timers = Timers::new(now);
    let menu = menu::Menu::new("CatPet");
    let w = render::WIN;
    let _ = std::fs::create_dir_all(dir);

    let colors = ["orange", "black", "brown", "white"];
    let moods = [
        ("idle", Mood::Idle),
        ("petted", Mood::Petted),
        ("typing", Mood::Typing),
        ("workdone", Mood::WorkDone),
    ];
    for color in colors {
        for (mname, mood) in moods {
            let mut st = CatState::new(now);
            st.clock = 0.5;
            st.set_mood(mood, Duration::from_secs(5), now);
            // Make eyes look toward the lower-right so tracking is visible.
            st.set_look(0.9, 0.5);
            if matches!(mood, Mood::Petted) {
                st.show_bubble("purr~", Duration::from_secs(5), now);
            }
            if matches!(mood, Mood::WorkDone) {
                st.trigger_hop();
                st.show_bubble("meow~", Duration::from_secs(5), now);
            }
            let mut cfg = Config::default();
            cfg.color_name = color.to_string();
            let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
            let path = format!("{dir}/cat_{color}_{mname}.png");
            let _ = pm.save_png(&path);
        }
    }
    // Rick character samples, including his two special poses.
    {
        let mut cfg = Config::default();
        cfg.character = "rick".into();

        let mut st = CatState::new(now);
        st.clock = 0.5;
        st.set_look(0.9, 0.5);
        let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
        let _ = pm.save_png(&format!("{dir}/rick_idle.png"));

        // Typing pose: sample two clock points so the hand-bob frames differ.
        for (name, clk) in [("a", 0.05f32), ("b", 0.35)] {
            let mut st = CatState::new(now);
            st.set_mood(Mood::Typing, Duration::from_secs(5), now);
            st.bump_energy(0.4);
            st.clock = clk;
            let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
            let _ = pm.save_png(&format!("{dir}/rick_typing_{name}.png"));
        }

        // Drag pose: Rick hanging from the hook.
        let mut st = CatState::new(now);
        st.start_drag();
        st.clock = 0.3;
        st.tick(now, 0.05);
        let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
        let _ = pm.save_png(&format!("{dir}/rick_drag.png"));
    }
    // Mochi-drag stretched pose (orange), eyes looking left.
    {
        let mut st = CatState::new(now);
        st.clock = 0.5;
        st.start_drag();
        st.tick(now, 0.1);
        st.set_look(-0.9, -0.4);
        let mut cfg = Config::default();
        cfg.color_name = "orange".into();
        let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
        let _ = pm.save_png(&format!("{dir}/cat_orange_mochi.png"));
    }
    // Eye-look direction sweep (orange), to verify only-eyes tracking.
    let looks = [
        ("c", 0.0f32, 0.0f32),
        ("l", -1.0, 0.0),
        ("r", 1.0, 0.0),
        ("u", 0.0, -1.0),
        ("d", 0.0, 1.0),
    ];
    for (name, lx, ly) in looks {
        let mut st = CatState::new(now);
        st.clock = 0.5;
        st.set_look(lx, ly);
        let mut cfg = Config::default();
        cfg.color_name = "orange".into();
        let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
        let _ = pm.save_png(&format!("{dir}/look_{name}.png"));
    }
    // Menu open (root) and menu with a submenu expanded.
    {
        let st = CatState::new(now);
        let cfg = Config::default();
        let mut m = menu::Menu::new("CatPet v0.1.0");
        m.open_at(70.0, 40.0);
        m.on_move(76.0, 40.0 + menu::HEADER_H + menu::PAD + 4.0); // hover first item
        let (bw, bh) = m.bounds_from_origin();
        let cw = (m.origin.0 + bw).ceil() as u32;
        let ch = (m.origin.1 + bh).max(render::WIN as f32).ceil() as u32;
        let pm = render::render(&st, &timers, &cfg, &sprites, &m, cw.max(render::WIN), ch, now);
        let _ = pm.save_png(&format!("{dir}/menu_root.png"));

        // Expand the Cat color submenu (index 0).
        let (ix, iy, iw, ih) = m.item_rect(0);
        m.on_move(ix + iw * 0.5, iy + ih * 0.5);
        let (bw, bh) = m.bounds_from_origin();
        let cw = (m.origin.0 + bw).ceil() as u32;
        let ch = (m.origin.1 + bh).max(render::WIN as f32).ceil() as u32;
        let pm = render::render(&st, &timers, &cfg, &sprites, &m, cw.max(render::WIN), ch, now);
        let _ = pm.save_png(&format!("{dir}/menu_sub.png"));
    }
    println!("dumped frames to {dir}");
}

fn send_or_hint(line: &str) {
    match ipc::send(line) {
        Ok(()) => {}
        Err(_) => {
            eprintln!(
                "[catpet] no running pet found. Start one with `catpet` first, \
                 then run `catpet {}`.",
                line.split_whitespace().next().unwrap_or("")
            );
        }
    }
}

fn print_help() {
    println!(
        "catpet — desktop cat pet\n\
         \n\
         catpet                 run the pet\n\
         catpet meow            work-done jump + meow\n\
         catpet pomodoro        toggle pomodoro (25/5)\n\
         catpet stretch         stretch reminder now\n\
         catpet color <name>    orange black brown white\n\
         catpet character <c>   cat | rick\n\
         catpet quit            close the pet\n\
         \n\
         Drag the cat with the left mouse button. Hover it to pet it."
    );
}

/// User-facing messages sent into the winit event loop from worker threads.
#[derive(Debug)]
enum AppMsg {
    Input(InputEvent),
    Ipc(IpcCmd),
    Frame, // redraw tick
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    _context: Option<Context<Rc<Window>>>,

    state: CatState,
    timers: Timers,
    cfg: Config,
    sprites: sprite::Sprites,
    menu: menu::Menu,
    /// User's name, used in reminders / "tell my name". Loaded from config.
    user_name: String,

    // Kept so worker-thread wiring can be extended to nudge the loop directly.
    #[allow(dead_code)]
    proxy: EventLoopProxy<AppMsg>,
    rx: Receiver<AppMsg>,

    last_frame: Instant,
    last_key: Instant,
    last_scroll: Instant,
    win_pos: (i32, i32),
    /// Current window size (grows while the menu is open).
    win_size: (u32, u32),
    dragging: bool,
    drag_grab: (f64, f64),
    cursor_screen: (f64, f64),
    /// Cursor position in window-local coords (from winit CursorMoved).
    cursor_local: (f64, f64),
    assets: AssetPaths,
}

struct AssetPaths {
    meow: std::path::PathBuf,
    chime: std::path::PathBuf,
}

impl App {
    fn apply_ipc(&mut self, cmd: IpcCmd, now: Instant) {
        match cmd {
            IpcCmd::Meow => {
                self.state.set_mood(Mood::WorkDone, Duration::from_millis(2200), now);
                self.state.trigger_hop();
                self.state.show_bubble("meow~", Duration::from_millis(2200), now);
                sound::play(&self.assets.meow);
            }
            IpcCmd::TogglePomodoro => {
                if self.timers.pomodoro_active() {
                    self.timers.stop_pomodoro();
                    self.state.show_bubble("stop", Duration::from_millis(1500), now);
                } else {
                    self.timers.start_pomodoro(now);
                    self.state.set_mood(Mood::Pomodoro, Duration::from_millis(1500), now);
                    self.state.show_bubble("focus!", Duration::from_millis(1800), now);
                }
            }
            IpcCmd::Stretch => {
                self.state.set_mood(Mood::Stretch, Duration::from_millis(2500), now);
                self.state.show_bubble("stretch!", Duration::from_millis(2500), now);
                sound::play(&self.assets.chime);
            }
            IpcCmd::SetColor(name) => {
                // Accept any known sprite colour (orange/black/brown/white);
                // sheet_for() maps aliases gracefully.
                self.cfg.color_name = name;
                self.cfg.save();
            }
            IpcCmd::SetCharacter(name) => {
                // "cat" | "rick"; sheet() treats unknown values as cat.
                self.cfg.character = name;
                self.cfg.save();
            }
            IpcCmd::SetPattern(_name) => {
                // Patterns don't apply to the fixed sprite art; accepted as a
                // no-op so legacy `catpet pattern X` calls don't error.
            }
            IpcCmd::SetUserName(name) => {
                let name = name.trim().to_string();
                save_name(&name);
                self.user_name = name.clone();
                self.state
                    .show_bubble(format!("hi {name}!"), Duration::from_millis(2200), now);
            }
            IpcCmd::Quit => {
                std::process::exit(0);
            }
        }
    }

    /// Open the context menu at window-local (lx, ly) and grow the window so the
    /// menu fits. The cat stays put on screen (window grows right + up).
    fn open_menu(&mut self, lx: f32, ly: f32) {
        // Anchor the menu near the cursor but keep it on the cat's right side so
        // it reads like ComNyang's. Clamp origin into the base cat block area.
        let base = render::WIN as f32;
        let ox = lx.clamp(20.0, base - 40.0);
        let oy = ly.clamp(10.0, base - 40.0);
        self.menu.open_at(ox, oy);
        self.grow_window_for_menu();
    }

    fn close_menu(&mut self) {
        self.menu.close();
        self.resize_window(render::WIN, render::WIN);
    }

    /// Resize the window to fit the currently-open menu. Because the cat lives in
    /// the bottom-left WIN block, we grow width to the right and height upward,
    /// shifting the window's top-left up so the cat doesn't jump.
    fn grow_window_for_menu(&mut self) {
        let base = render::WIN as f32;
        let (bw, bh) = self.menu.bounds_from_origin();
        // Menu origin is inside the cat block; needed canvas is origin + bounds.
        let need_w = (self.menu.origin.0 + bw).max(base).ceil() as u32;
        let need_h = (self.menu.origin.1 + bh).max(base).ceil() as u32;
        // Only ever grow within reason; menu shouldn't exceed ~2x.
        let cw = need_w.max(render::WIN);
        let ch = need_h.max(render::WIN);
        self.resize_window(cw, ch);
    }

    /// Set the window to (cw, ch), keeping the cat's bottom-left corner fixed on
    /// screen by moving the window's top-left up as it grows taller.
    fn resize_window(&mut self, cw: u32, ch: u32) {
        let (old_w, old_h) = self.win_size;
        if (cw, ch) == (old_w, old_h) {
            return;
        }
        // Bottom edge stays fixed: new_top = old_top + (old_h - new_h).
        let dy = old_h as i32 - ch as i32;
        let new_y = self.win_pos.1 + dy;
        self.win_size = (cw, ch);
        self.win_pos = (self.win_pos.0, new_y);
        if let Some(w) = &self.window {
            let _ = w.request_inner_size(PhysicalSize::new(cw, ch));
            w.set_outer_position(PhysicalPosition::new(self.win_pos.0, new_y));
            w.request_redraw();
        }
    }

    /// Perform a menu action.
    fn run_action(&mut self, act: menu::Action, now: Instant) {
        use menu::Action;
        // Any action closes the menu and shrinks the window.
        match act {
            Action::TogglePomodoro => {
                if self.timers.pomodoro_active() {
                    self.timers.stop_pomodoro();
                    self.state.show_bubble("stop", Duration::from_millis(1400), now);
                } else {
                    self.timers.start_pomodoro(now);
                    self.state.show_bubble("focus!", Duration::from_millis(1600), now);
                }
            }
            Action::StretchNow | Action::BreakStretchToggle => {
                self.state.set_mood(Mood::Stretch, Duration::from_millis(2500), now);
                self.state.show_bubble("stretch!", Duration::from_millis(2500), now);
                sound::play(&self.assets.chime);
            }
            Action::ShowOff => {
                self.state.set_mood(Mood::WorkDone, Duration::from_millis(2000), now);
                self.state.trigger_hop();
                self.state.show_bubble("ta-da!", Duration::from_millis(2000), now);
                sound::play(&self.assets.meow);
            }
            Action::TellName => {
                let msg = if self.user_name.is_empty() {
                    "who are you?".to_string()
                } else {
                    format!("hi {}!", self.user_name)
                };
                self.state.show_bubble(msg, Duration::from_millis(2200), now);
            }
            Action::SetName => {
                // Full text entry needs a popup; for now cycle a hint.
                self.state
                    .show_bubble("set via: catpet name X", Duration::from_millis(2600), now);
            }
            Action::ShowName => {
                let msg = if self.user_name.is_empty() {
                    "no name yet".to_string()
                } else {
                    self.user_name.clone()
                };
                self.state.show_bubble(msg, Duration::from_millis(2000), now);
            }
            Action::FixedMessagePin => {
                self.state.show_bubble("pinned!", Duration::from_millis(1800), now);
            }
            Action::FixedMessageClear => {
                self.state.show_bubble("cleared", Duration::from_millis(1400), now);
            }
            Action::OpenReminders => {
                self.state
                    .show_bubble("reminders soon", Duration::from_millis(2000), now);
            }
            Action::SetColor(c) => {
                self.cfg.color_name = c.as_str().to_string();
                self.cfg.save();
                self.state
                    .show_bubble(c.as_str(), Duration::from_millis(1400), now);
            }
            Action::SetCharacter(c) => {
                self.cfg.character = c.as_str().to_string();
                self.cfg.save();
                self.state
                    .show_bubble(c.as_str(), Duration::from_millis(1400), now);
            }
            Action::Quit => std::process::exit(0),
        }
        self.close_menu();
    }

    fn apply_input(&mut self, ev: InputEvent, now: Instant) {
        match ev {
            InputEvent::MouseMove { x, y } => {
                let (px, py) = self.cursor_screen;
                self.cursor_screen = (x, y);

                let (wx, wy) = self.win_pos;
                let win = render::WIN as f64;

                // The cat's head sits low-centre in the window. Track that point
                // for eye aim and for the "pet the head" hit test.
                let head_x = wx as f64 + win * 0.5;
                let head_y = wy as f64 + win * 0.78;

                // Eyes look toward the cursor (only the eyes move; body is fixed).
                let dx = ((x - head_x) / 180.0) as f32;
                let dy = ((y - head_y) / 180.0) as f32;
                self.state.set_look(dx, dy);

                if self.dragging {
                    // Feed shake speed into the wobble.
                    let speed = (((x - px).powi(2) + (y - py).powi(2)).sqrt()) as f32;
                    self.state.drag_move(speed);
                } else {
                    // Pet the head: cursor within a small radius of the head point.
                    let dist = ((x - head_x).powi(2) + (y - head_y).powi(2)).sqrt();
                    if dist < win * 0.28 {
                        self.state.set_mood(Mood::Petted, Duration::from_millis(400), now);
                    }
                }
            }
            InputEvent::KeyPress => {
                self.last_key = now;
                self.state.set_mood(Mood::Typing, Duration::from_millis(500), now);
                self.state.bump_energy(0.15);
            }
            InputEvent::Scroll => {
                self.last_scroll = now;
                self.state.set_mood(Mood::Scrolling, Duration::from_millis(450), now);
            }
            InputEvent::Click => {
                // A click while hovering gives a happy blink.
                self.state.set_mood(Mood::Petted, Duration::from_millis(700), now);
            }
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        if surface
            .resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap())
            .is_err()
        {
            return;
        }

        let now = Instant::now();
        let pm = render::render(
            &self.state,
            &self.timers,
            &self.cfg,
            &self.sprites,
            &self.menu,
            w,
            h,
            now,
        );

        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        // tiny-skia pixmap is premultiplied RGBA bytes; softbuffer wants
        // 0x00RRGGBB with the alpha implied by the compositor. For a transparent
        // window we pack ARGB where A drives per-pixel transparency.
        let data = pm.data();
        let px_count = (w * h) as usize;
        for i in 0..px_count.min(buffer.len()) {
            let o = i * 4;
            if o + 3 >= data.len() {
                break;
            }
            let r = data[o] as u32;
            let g = data[o + 1] as u32;
            let b = data[o + 2] as u32;
            let a = data[o + 3] as u32;
            buffer[i] = (a << 24) | (r << 16) | (g << 8) | b;
        }
        let _ = buffer.present();
    }
}

impl ApplicationHandler<AppMsg> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("catpet")
            .with_inner_size(PhysicalSize::new(render::WIN, render::WIN))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));

        // Start bottom-right-ish.
        if let Some(mon) = window.current_monitor() {
            let msize = mon.size();
            let x = msize.width as i32 - render::WIN as i32 - 60;
            let y = msize.height as i32 - render::WIN as i32 - 90;
            window.set_outer_position(PhysicalPosition::new(x, y));
            self.win_pos = (x, y);
        }

        let context = Context::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");

        self.window = Some(window);
        self.surface = Some(surface);
        self._context = Some(context);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, msg: AppMsg) {
        let now = Instant::now();
        match msg {
            AppMsg::Input(ev) => self.apply_input(ev, now),
            AppMsg::Ipc(cmd) => self.apply_ipc(cmd, now),
            AppMsg::Frame => {}
        }
        // Drain any queued channel messages too (input can be bursty).
        while let Ok(m) = self.rx.try_recv() {
            match m {
                AppMsg::Input(ev) => self.apply_input(ev, now),
                AppMsg::Ipc(cmd) => self.apply_ipc(cmd, now),
                AppMsg::Frame => {}
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                use winit::event::{ElementState, MouseButton};
                let now = Instant::now();
                match (button, btn_state) {
                    (MouseButton::Right, ElementState::Pressed) => {
                        // Open the context menu at the cursor, grow the window.
                        let (lx, ly) = self.cursor_local;
                        self.open_menu(lx as f32, ly as f32);
                    }
                    (MouseButton::Left, ElementState::Pressed) => {
                        if self.menu.open {
                            // Route click into the menu (action or close).
                            let (lx, ly) = self.cursor_local;
                            if let Some(act) = self.menu.on_click(lx as f32, ly as f32) {
                                self.run_action(act, now);
                            }
                            if !self.menu.open {
                                self.close_menu();
                            }
                        } else {
                            self.dragging = true;
                            self.drag_grab = self.cursor_screen;
                            self.state.start_drag(); // mochi: lift + stretch
                        }
                    }
                    (MouseButton::Left, ElementState::Released) => {
                        if self.dragging {
                            self.dragging = false;
                            self.state.end_drag(); // mochi: land + spring back
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_local = (position.x, position.y);
                if self.menu.open {
                    let prev_sub = self.menu.open_sub;
                    self.menu.on_move(position.x as f32, position.y as f32);
                    // If a submenu expanded/changed, the window may need more room.
                    if self.menu.open_sub != prev_sub {
                        self.grow_window_for_menu();
                    }
                }
                if self.dragging {
                    let (gx, gy) = self.cursor_screen;
                    let dx = (gx - self.drag_grab.0) as i32;
                    let dy = (gy - self.drag_grab.1) as i32;
                    let nx = self.win_pos.0 + dx;
                    let ny = self.win_pos.1 + dy;
                    self.win_pos = (nx, ny);
                    self.drag_grab = self.cursor_screen;
                    if let Some(w) = &self.window {
                        w.set_outer_position(PhysicalPosition::new(nx, ny));
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        self.state.tick(now, dt);

        // Timer events.
        for ev in self.timers.poll(now) {
            match ev {
                TimerEvent::Stretch => {
                    self.state.set_mood(Mood::Stretch, Duration::from_millis(3000), now);
                    self.state.show_bubble("stretch!", Duration::from_millis(3000), now);
                    sound::play(&self.assets.chime);
                }
                TimerEvent::WorkStarted => {
                    self.state.set_mood(Mood::Pomodoro, Duration::from_millis(2000), now);
                    self.state.show_bubble("focus!", Duration::from_millis(2200), now);
                    sound::play(&self.assets.chime);
                }
                TimerEvent::BreakStarted => {
                    self.state.set_mood(Mood::Pomodoro, Duration::from_millis(2000), now);
                    self.state.show_bubble("break~", Duration::from_millis(2200), now);
                    sound::play(&self.assets.chime);
                }
            }
        }

        // While a pomodoro runs, keep the countdown label visible.
        if let Some(label) = self.timers.label(now) {
            if self.state.bubble.is_none() {
                self.state.show_bubble(label, Duration::from_millis(1100), now);
            }
        }

        if let Some(w) = &self.window {
            w.request_redraw();
        }
        // ~30 fps.
        event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(33)));
    }
}

fn run_pet() {
    let now = Instant::now();
    let event_loop: EventLoop<AppMsg> = EventLoop::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();

    // Channel that worker threads push into; we also nudge the loop via the proxy.
    let (tx, rx) = channel::<AppMsg>();

    // Global input thread -> AppMsg::Input
    {
        let (itx, irx) = channel::<InputEvent>();
        input::spawn(itx);
        let tx2 = tx.clone();
        let proxy2 = proxy.clone();
        std::thread::spawn(move || {
            for ev in irx {
                if tx2.send(AppMsg::Input(ev)).is_err() {
                    break;
                }
                let _ = proxy2.send_event(AppMsg::Frame);
            }
        });
    }

    // IPC thread -> AppMsg::Ipc
    {
        let (ctx, crx) = channel::<IpcCmd>();
        let proxy_ipc = proxy.clone();
        ipc::serve(ctx, move || {
            let _ = proxy_ipc.send_event(AppMsg::Frame);
        });
        let tx3 = tx.clone();
        let proxy3 = proxy.clone();
        std::thread::spawn(move || {
            for cmd in crx {
                if tx3.send(AppMsg::Ipc(cmd)).is_err() {
                    break;
                }
                let _ = proxy3.send_event(AppMsg::Frame);
            }
        });
    }

    let assets = AssetPaths {
        meow: asset_dir().join("meow.wav"),
        chime: asset_dir().join("chime.wav"),
    };

    let mut app = App {
        window: None,
        surface: None,
        _context: None,
        state: CatState::new(now),
        timers: Timers::new(now),
        cfg: Config::load(),
        sprites: sprite::Sprites::load(),
        menu: menu::Menu::new(concat!("CatPet v", env!("CARGO_PKG_VERSION"))),
        user_name: load_name(),
        proxy,
        rx,
        last_frame: now,
        last_key: now,
        last_scroll: now,
        win_pos: (0, 0),
        win_size: (render::WIN, render::WIN),
        dragging: false,
        drag_grab: (0.0, 0.0),
        cursor_screen: (0.0, 0.0),
        cursor_local: (0.0, 0.0),
        assets,
    };

    event_loop.run_app(&mut app).expect("run app");
}

fn asset_dir() -> std::path::PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            std::path::PathBuf::from(home).join(".local/share")
        });
    base.join("catpet")
}

fn name_path() -> std::path::PathBuf {
    asset_dir().join("name.txt")
}

fn load_name() -> String {
    std::fs::read_to_string(name_path())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn save_name(name: &str) {
    let _ = std::fs::create_dir_all(asset_dir());
    let _ = std::fs::write(name_path(), name.trim());
}

/// Write bundled WAV sounds to the data dir on first run if missing.
fn ensure_assets() {
    let dir = asset_dir();
    let _ = std::fs::create_dir_all(&dir);
    let meow = dir.join("meow.wav");
    let chime = dir.join("chime.wav");
    if !meow.exists() {
        let _ = std::fs::write(&meow, synth_meow());
    }
    if !chime.exists() {
        let _ = std::fs::write(&chime, synth_chime());
    }
}

// --- Tiny WAV synthesis so we ship no binary audio assets ---

fn wav_header(n_samples: u32, sample_rate: u32) -> Vec<u8> {
    let byte_rate = sample_rate * 2;
    let data_len = n_samples * 2;
    let mut h = Vec::new();
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data_len).to_le_bytes());
    h.extend_from_slice(b"WAVE");
    h.extend_from_slice(b"fmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&1u16.to_le_bytes()); // mono
    h.extend_from_slice(&sample_rate.to_le_bytes());
    h.extend_from_slice(&byte_rate.to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes()); // block align
    h.extend_from_slice(&16u16.to_le_bytes()); // bits
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_len.to_le_bytes());
    h
}

/// A short two-tone "meow": pitch rises then falls, with an amplitude envelope.
fn synth_meow() -> Vec<u8> {
    let sr = 22050u32;
    let dur = 0.45f32;
    let n = (sr as f32 * dur) as u32;
    let mut samples: Vec<i16> = Vec::with_capacity(n as usize);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let p = t / dur;
        // Formant-ish glide: up then down (me-ow).
        let f = if p < 0.4 {
            480.0 + p * 500.0
        } else {
            700.0 - (p - 0.4) * 500.0
        };
        let vibrato = (t * 2.0 * std::f32::consts::PI * 6.0).sin() * 12.0;
        let phase = 2.0 * std::f32::consts::PI * (f + vibrato) * t;
        // Add a couple of harmonics for a catlike timbre.
        let s = phase.sin() * 0.6 + (phase * 2.0).sin() * 0.25 + (phase * 3.0).sin() * 0.1;
        // Envelope: soft attack, gentle decay.
        let env = (p * std::f32::consts::PI).sin().powf(0.6);
        let v = (s * env * 0.5 * i16::MAX as f32) as i16;
        samples.push(v);
    }
    let mut out = wav_header(n, sr);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// A soft two-note chime for stretch/pomodoro.
fn synth_chime() -> Vec<u8> {
    let sr = 22050u32;
    let dur = 0.5f32;
    let n = (sr as f32 * dur) as u32;
    let mut samples: Vec<i16> = Vec::with_capacity(n as usize);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let p = t / dur;
        let f = if p < 0.5 { 660.0 } else { 880.0 };
        let phase = 2.0 * std::f32::consts::PI * f * t;
        let s = phase.sin() * 0.5 + (phase * 2.0).sin() * 0.15;
        let env = (-(p) * 4.0).exp(); // pluck-like decay each note-ish
        let v = (s * env * 0.4 * i16::MAX as f32) as i16;
        samples.push(v);
    }
    let mut out = wav_header(n, sr);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}
