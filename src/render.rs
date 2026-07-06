//! Compositing renderer: blits a scaled pixel-cat sprite frame, then draws UI
//! overlays (Pomodoro ring, speech bubble) on top with tiny-skia.
//!
//! The sprite is the cat; overlays are vector. Everything lands in a WIN x WIN
//! premultiplied pixmap that `main` copies to the window buffer.

use crate::config::Config;
use crate::menu::{Menu, ITEM_H, HEADER_H, MENU_W, PAD};
use crate::sprite::{kind, CharacterKind, Facing, RickPose, Sprites, FRAME_H, FRAME_W};
use crate::state::{CatState, Mood};
use crate::timers::{Phase, Timers};
use std::time::Instant;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

pub const WIN: u32 = 220;

/// Integer upscale factor for the pixel sprite (32x48 -> 128x192).
const SCALE: u32 = 4;

/// Render into a canvas of `canvas_w x canvas_h`. The cat always occupies the
/// bottom-left WIN x WIN block; any extra width/height is space for the menu.
pub fn render(
    state: &CatState,
    timers: &Timers,
    cfg: &Config,
    sprites: &Sprites,
    menu: &Menu,
    canvas_w: u32,
    canvas_h: u32,
    now: Instant,
) -> Pixmap {
    let mut pm = Pixmap::new(canvas_w, canvas_h).unwrap();

    // The cat's sub-region sits at the bottom-left of the canvas.
    let cat_top = canvas_h as f32 - WIN as f32;
    let cx = WIN as f32 / 2.0;
    let breathe = (state.clock * 2.0).sin() * 1.0;

    let (facing, col) = pick_frame(state);

    // Rick has two dedicated full-body poses that replace the walk frame: he
    // hangs from a hook while dragged and hunches over a keyboard while typing.
    // For any other character these are None and nothing below changes.
    let rick_pose = if kind(&cfg.character) == CharacterKind::Rick {
        if state.dragging {
            Some(RickPose::Drag)
        } else if state.mood == Mood::Typing {
            Some(RickPose::Type)
        } else {
            None
        }
    } else {
        None
    };

    // Animation frame for the typing pose. The 4-frame strip is ordered
    // [left-hand up, neutral, right-hand up, neutral], so simply advancing
    // through it 0->1->2->3 alternates the tapping hands. Typing energy speeds
    // it up (faster typing = busier hands). Drag is a still pose, so its column
    // is irrelevant.
    let pose_col = ((state.clock * (7.0 + state.energy * 9.0)) as usize % 4) as u32;

    // Mochi squash: positive `squash` stretches tall + thin, negative squashes
    // wide + short, preserving footprint. Wobble shears horizontally. Rick's
    // pose art already carries its own posture (raised arm, hunch), so squashing
    // it would distort the hook/chain — hold it neutral while a pose is active.
    let sq = if rick_pose.is_some() {
        0.0
    } else {
        state.squash.clamp(-0.4, 0.5)
    };
    let scale_x = 1.0 - sq * 0.6;
    let scale_y = 1.0 + sq;
    let base_w = (FRAME_W * SCALE) as f32;
    let base_h = (FRAME_H * SCALE) as f32;
    let sw = base_w * scale_x;
    let sh = base_h * scale_y;
    let shear = (state.clock * 22.0).sin() * state.wobble * 10.0;

    let cxp = cx + shear;
    let sx = cxp - sw / 2.0;
    // Feet stay planted at the same baseline regardless of stretch. Baseline is
    // measured within the cat's bottom-left WIN block.
    let baseline = cat_top + WIN as f32 - 8.0;
    let sy = baseline - sh - state.hop - breathe;

    // Resolve the exact frame we're about to draw so the shadow can be anchored
    // to its real feet (which are neither at the frame bottom nor, for some art,
    // horizontally centred).
    let sheet = sprites.sheet(&cfg.character, &cfg.color_name);
    let frame = match rick_pose {
        Some(pose) => sprites.rick_pose(pose, pose_col),
        None => sheet.frame(facing, col),
    };

    // The shadow sits under the figure's FEET. Vertically the art leaves empty
    // margin below the feet (cat feet ~88% down the frame, rick ~75%);
    // horizontally the figure may be off-centre in its cell. Anchoring to the
    // actual feet keeps the shadow glued under the character in every pose.
    let feet_frac = feet_fraction(&cfg.character);
    let feet_y = sy + feet_frac * sh;
    let feet_x = sx + crate::sprite::feet_center_frac(frame) * sw;
    // Center the shadow a few px below the feet so the character stands ON it.
    draw_shadow(&mut pm, feet_x, feet_y + 4.0, state.hop);

    // A little keyboard the cat kneads while typing (drawn behind the paws).
    // Rick brings his own keyboard in the pose art, so skip the cat overlay for
    // him — otherwise two keyboards stack.
    if state.mood == Mood::Typing && rick_pose.is_none() {
        draw_keyboard(&mut pm, state, cx, baseline);
    }

    // Blit the sprite, repainting its own eye pixels so the pupils look toward
    // the cursor (only the eyes move; everything stays in the pixel grid).
    let eyes = EyeAim {
        // ±1 pixel horizontal, ±1 pixel vertical, in sprite (source) pixels.
        dx: (state.look.0 * 1.6).round().clamp(-1.0, 1.0) as i32,
        dy: (state.look.1 * 1.4).round().clamp(-1.0, 1.0) as i32,
        blink: state.blink,
        happy: state.mood == Mood::Petted,
    };
    blit_sprite(&mut pm, frame, cfg, facing, sx, sy, scale_x, scale_y, eyes, rick_pose);

    if state.mood == Mood::Petted {
        draw_hearts(&mut pm, state, cxp, sy);
    }
    if state.mood == Mood::Typing && rick_pose.is_none() {
        draw_knead_paws(&mut pm, state, cx, baseline);
    }

    if let Some(remaining) = timers.remaining(now) {
        draw_pomodoro_ring(&mut pm, timers, remaining, cx, cat_top);
    }
    if let Some(bubble) = &state.bubble {
        draw_bubble(&mut pm, &bubble.text, cx, cat_top);
    }

    if menu.open {
        draw_menu(&mut pm, menu);
    }

    pm
}

/// Fraction down the sprite frame where the character's feet sit. The art leaves
/// empty space below the feet, and it differs per character (cat feet ~88% down,
/// rick ~75%), so the ground shadow is anchored here rather than the frame bottom.
fn feet_fraction(character: &str) -> f32 {
    match kind(character) {
        CharacterKind::Rick => 144.0 / 192.0, // measured from rick.png's front frame
        CharacterKind::Cat => 42.0 / 48.0,    // measured from the cat sheets
    }
}

/// Pick (facing, column) for the current mood + animation clock.
///
/// The pet ALWAYS faces the viewer (Facing::Down) so the cat's code-drawn eyes
/// can track the cursor independently — the body never turns to follow the
/// mouse. Only column (the small in-place step) varies with mood; Rick's
/// front row carries a marching step so the same cycle reads as walking.
fn pick_frame(state: &CatState) -> (Facing, u32) {
    let cycle = [0u32, 1, 2, 1];
    let step = |speed: f32| cycle[((state.clock * speed) as usize) % 4];

    let col = match state.mood {
        // Knead: quick paw shuffle in place while typing.
        Mood::Typing => step(8.0 + state.energy * 8.0),
        Mood::Scrolling => step(6.0),
        Mood::WorkDone => step(12.0),
        Mood::Stretch => step(3.0),
        Mood::Pomodoro | Mood::Petted => 1,
        Mood::Idle => {
            // Occasional slow breathing shuffle; otherwise stand.
            if (state.clock * 0.5).sin() > 0.92 {
                step(4.0)
            } else {
                1
            }
        }
    };
    (Facing::Down, col)
}

/// How the pupils should be repainted this frame.
#[derive(Clone, Copy)]
struct EyeAim {
    dx: i32,   // pupil shift in sprite pixels, -1..1
    dy: i32,   // pupil shift in sprite pixels, -1..1
    blink: f32, // 0 open .. 1 shut
    happy: bool, // squint when petted
}

fn blit_sprite(
    pm: &mut Pixmap,
    base: &image::RgbaImage,
    cfg: &Config,
    facing: Facing,
    sx: f32,
    sy: f32,
    scale_x: f32,
    scale_y: f32,
    eyes: EyeAim,
    rick_pose: Option<RickPose>,
) {
    // Patch the cat's own eye pixels so the pupils look toward the cursor. Only
    // done on the front (Down) frame, which is the only one with visible eyes,
    // and never on a Rick pose (those aren't cat frames).
    let patched = if rick_pose.is_none()
        && matches!(facing, Facing::Down)
        && kind(&cfg.character) == CharacterKind::Cat
    {
        Some(patch_eyes(base, eyes))
    } else {
        None
    };
    let frame: &image::RgbaImage = patched.as_ref().unwrap_or(base);

    let data = pm.data_mut();
    let win = WIN as i32;

    // Destination size after squash. Nearest-neighbour sample from the source
    // frame by destination ratio so the sprite stretches/squashes as a whole.
    // The on-screen box is fixed (FRAME_W x FRAME_H * SCALE); the frame's own
    // resolution can differ per character (rick ships at 4x the cat's).
    let fw = frame.width();
    let fh = frame.height();
    let dw = ((FRAME_W * SCALE) as f32 * scale_x).round().max(1.0) as i32;
    let dh = ((FRAME_H * SCALE) as f32 * scale_y).round().max(1.0) as i32;

    for py in 0..dh {
        let src_y = ((py as f32 / dh as f32) * fh as f32) as u32;
        let src_y = src_y.min(fh - 1);
        let dy = sy as i32 + py;
        if dy < 0 || dy >= win {
            continue;
        }
        for px in 0..dw {
            let src_x = ((px as f32 / dw as f32) * fw as f32) as u32;
            let src_x = src_x.min(fw - 1);
            let sp = frame.get_pixel(src_x, src_y).0;
            let a = sp[3] as u32;
            if a == 0 {
                continue;
            }
            let dx = sx as i32 + px;
            if dx < 0 || dx >= win {
                continue;
            }
            let di = ((dy * win + dx) * 4) as usize;
            // Premultiply source.
            let sr = sp[0] as u32 * a / 255;
            let sg = sp[1] as u32 * a / 255;
            let sb = sp[2] as u32 * a / 255;
            // Source-over onto existing premultiplied dst.
            let inv = 255 - a;
            let dr = data[di] as u32;
            let dg = data[di + 1] as u32;
            let db = data[di + 2] as u32;
            let da = data[di + 3] as u32;
            data[di] = (sr + dr * inv / 255) as u8;
            data[di + 1] = (sg + dg * inv / 255) as u8;
            data[di + 2] = (sb + db * inv / 255) as u8;
            data[di + 3] = (a + da * inv / 255) as u8;
        }
    }
}

/// The two baked eye pixels on the front frame (measured from the art). Each eye
/// is a vertical 2px dot; we treat the upper pixel as the anchor and shift the
/// dark "pupil" within a small socket around it.
const EYE_L: (i32, i32) = (13, 34); // left eye, upper pixel
const EYE_R: (i32, i32) = (17, 34); // right eye, upper pixel
/// The eyes span rows 34..=35; the socket we may paint into is rows 33..=36.
const EYE_ROWS: (i32, i32) = (34, 35);

/// Return a copy of `base` with the cat's own eyes repainted so the pupils look
/// toward the cursor (dx,dy in sprite pixels). Blink closes them; happy squints.
///
/// Strategy: sample a nearby fur colour, clear the original 2px eye of each side
/// back to fur, then stamp a single dark pupil pixel at the shifted position —
/// all on the sprite's native pixel grid so it still reads as pixel art.
fn patch_eyes(base: &image::RgbaImage, eyes: EyeAim) -> image::RgbaImage {
    let mut img = base.clone();

    // Sample fur just above the eyes (row 33 is muzzle fur between/around eyes).
    let fur_at = |x: i32| -> [u8; 4] {
        let sx = x.clamp(0, FRAME_W as i32 - 1) as u32;
        base.get_pixel(sx, 33).0
    };

    // The dark pupil colour: reuse the sprite's own darkest eye tone.
    let pupil = [40u8, 36, 42, 255];

    for &(ex, ey) in &[EYE_L, EYE_R] {
        // 1) Clear the original 2px eye back to fur.
        let fur = fur_at(ex);
        for yy in EYE_ROWS.0..=EYE_ROWS.1 {
            put(&mut img, ex, yy, fur);
        }

        if eyes.blink > 0.55 || eyes.happy {
            // Closed / squint: draw a 1px dark line across the eye row (a content
            // cat with eyes shut), leave it at that.
            put(&mut img, ex, ey, pupil);
            continue;
        }

        // 2) Stamp the pupil, shifted toward the cursor but kept in the socket.
        let px = (ex + eyes.dx).clamp(ex - 1, ex + 1);
        let py = (ey + eyes.dy).clamp(EYE_ROWS.0 - 1, EYE_ROWS.1);
        put(&mut img, px, py, pupil);
    }

    img
}

/// Write a pixel if in bounds.
fn put(img: &mut image::RgbaImage, x: i32, y: i32, rgba: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, image::Rgba(rgba));
    }
}

fn oval(pm: &mut Pixmap, cx: f32, cy: f32, rx: f32, ry: f32, col: Color) {
    let mut pb = PathBuilder::new();
    if let Some(rect) = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0) {
        pb.push_oval(rect);
    }
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(col);
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// A small pixel keyboard under the cat's paws, shown while typing.
fn draw_keyboard(pm: &mut Pixmap, state: &CatState, cx: f32, baseline: f32) {
    let w = 92.0;
    let h = 20.0;
    let x = cx - w / 2.0;
    let y = baseline - h + 4.0;

    // Body.
    round_rect(pm, x, y, w, h, 4.0, Color::from_rgba8(60, 64, 74, 240));
    round_rect(pm, x + 2.0, y + 2.0, w - 4.0, h - 5.0, 3.0, Color::from_rgba8(84, 90, 104, 240));

    // Keys grid; the two keys under the active paw "press" (darken + drop).
    let cols = 8;
    let rows = 2;
    let kw = (w - 10.0) / cols as f32;
    let kh = (h - 9.0) / rows as f32;
    let phase = (state.clock * (10.0 + state.energy * 12.0)) as usize;
    for r in 0..rows {
        for c in 0..cols {
            let kx = x + 5.0 + c as f32 * kw;
            let ky = y + 4.0 + r as f32 * kh;
            let pressed = (c + r) % cols == phase % cols;
            let (drop, shade) = if pressed {
                (1.5, Color::from_rgba8(150, 200, 235, 255))
            } else {
                (0.0, Color::from_rgba8(210, 214, 222, 255))
            };
            round_rect(pm, kx, ky + drop, kw - 2.0, kh - 2.0, 1.5, shade);
        }
    }
}

/// Two little paws kneading on top of the keyboard while typing.
fn draw_knead_paws(pm: &mut Pixmap, state: &CatState, cx: f32, baseline: f32) {
    let s = state.clock * (10.0 + state.energy * 12.0);
    let lift_l = s.sin().max(0.0) * 6.0;
    let lift_r = (s + std::f32::consts::PI).sin().max(0.0) * 6.0;
    let paw_y = baseline - 20.0;
    let col = Color::from_rgba8(245, 238, 230, 255);
    oval(pm, cx - 16.0, paw_y - lift_l, 8.0, 6.0, col);
    oval(pm, cx + 16.0, paw_y - lift_r, 8.0, 6.0, col);
    // Toe beans.
    let bean = Color::from_rgba8(230, 150, 160, 255);
    oval(pm, cx - 16.0, paw_y - lift_l + 2.0, 2.0, 1.6, bean);
    oval(pm, cx + 16.0, paw_y - lift_r + 2.0, 2.0, 1.6, bean);
}

fn round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, col: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(col);
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_shadow(pm: &mut Pixmap, cx: f32, cy: f32, hop: f32) {
    let shrink = (hop / 22.0).clamp(0.0, 1.0);
    let rx = 40.0 - shrink * 12.0;
    let a = (65.0 - shrink * 35.0) as u8;
    let mut pb = PathBuilder::new();
    let rect = Rect::from_xywh(cx - rx, cy - 7.0, rx * 2.0, 14.0).unwrap();
    pb.push_oval(rect);
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(0, 0, 0, a));
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_hearts(pm: &mut Pixmap, state: &CatState, cx: f32, top: f32) {
    // Two little pulsing hearts floating above the head when petted.
    let pulse = (state.clock * 4.0).sin() * 0.5 + 0.5;
    let a = (150.0 + pulse * 105.0) as u8;
    let col = Color::from_rgba8(0xF0, 0x6C, 0x8A, a);
    for (i, dx) in [(-1.0f32, -18.0), (1.0, 16.0)].iter() {
        let hx = cx + dx;
        let hy = top + 6.0 - i * 4.0 - pulse * 6.0;
        heart(pm, hx, hy, 5.0, col);
    }
}

fn heart(pm: &mut Pixmap, x: f32, y: f32, s: f32, col: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(x, y + s * 0.9);
    pb.cubic_to(x - s, y, x - s * 0.6, y - s, x, y - s * 0.3);
    pb.cubic_to(x + s * 0.6, y - s, x + s, y, x, y + s * 0.9);
    pb.close();
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(col);
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_pomodoro_ring(pm: &mut Pixmap, timers: &Timers, remaining: std::time::Duration, cx: f32, cat_top: f32) {
    let (total, color) = match timers.phase {
        Phase::Work => (25.0 * 60.0, Color::from_rgba8(0xE0, 0x6C, 0x5A, 255)),
        Phase::Break => (5.0 * 60.0, Color::from_rgba8(0x5A, 0xC0, 0x8A, 255)),
        Phase::Off => return,
    };
    let frac = (remaining.as_secs_f32() / total).clamp(0.0, 1.0);
    let cy = cat_top + 24.0;
    let r = 15.0;
    let segments = 24;
    let lit = (frac * segments as f32).ceil() as i32;
    for i in 0..segments {
        let ang = std::f32::consts::PI * (1.0 + i as f32 / (segments - 1) as f32);
        let dx = cx + ang.cos() * r;
        let dy = cy + ang.sin() * r;
        let on = i < lit;
        let col = if on {
            color
        } else {
            Color::from_rgba8(200, 200, 205, 90)
        };
        dot(pm, dx, dy, 2.1, col);
    }
}

fn dot(pm: &mut Pixmap, x: f32, y: f32, r: f32, col: Color) {
    let mut pb = PathBuilder::new();
    let rect = Rect::from_xywh(x - r, y - r, r * 2.0, r * 2.0).unwrap();
    pb.push_oval(rect);
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(col);
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_bubble(pm: &mut Pixmap, text: &str, cx: f32, cat_top: f32) {
    let w = (text.len() as f32 * 8.0 + 16.0).min(200.0);
    let h = 22.0;
    let x = (cx - w / 2.0).max(4.0);
    let y = cat_top + 3.0;

    // Rounded body.
    let mut pb = PathBuilder::new();
    let r = 9.0;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(255, 255, 255, 240));
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
    // Tail.
    let mut tb = PathBuilder::new();
    tb.move_to(cx - 5.0, y + h - 1.0);
    tb.line_to(cx + 5.0, y + h - 1.0);
    tb.line_to(cx, y + h + 7.0);
    tb.close();
    if let Some(p) = tb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(255, 255, 255, 240));
        paint.anti_alias = true;
        pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }

    draw_text(pm, text, x + 8.0, y + 6.0);
}

/// Ultra-light 3x5 block font.
fn draw_text(pm: &mut Pixmap, text: &str, x0: f32, y0: f32) {
    draw_text_c(pm, text, x0, y0, 1.7, Color::from_rgba8(60, 60, 70, 255));
}

/// Block-font text with explicit pixel size and colour.
fn draw_text_c(pm: &mut Pixmap, text: &str, x0: f32, y0: f32, px: f32, col: Color) {
    let mut paint = Paint::default();
    paint.set_color(col);
    paint.anti_alias = false;
    let mut cx = x0;
    for ch in text.to_ascii_uppercase().chars() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for c in 0..3 {
                if bits & (1 << (2 - c)) != 0 {
                    let rx = cx + c as f32 * px;
                    let ry = y0 + row as f32 * px;
                    if let Some(rect) = Rect::from_xywh(rx, ry, px, px) {
                        pm.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                }
            }
        }
        cx += 4.0 * px;
    }
}

/// Draw the right-click context menu (dark rounded pixel panel) using the
/// readable 5x7 font.
fn draw_menu(pm: &mut Pixmap, menu: &Menu) {
    use crate::font;
    let (ox, oy) = menu.origin;
    let panel_h = menu.panel_h();

    let bg = Color::from_rgba8(34, 36, 44, 250);
    let bg_hi = Color::from_rgba8(74, 122, 210, 255); // hover highlight
    let txt = Color::from_rgba8(236, 238, 244, 255);
    let txt_dim = Color::from_rgba8(150, 154, 166, 255);

    // Vertical centring of a 7px-tall glyph within an ITEM_H row at scale 2.
    let fs = 2.0; // font pixel scale
    let glyph_h = font::GLYPH_H as f32 * fs;
    let text_dy = (ITEM_H - glyph_h) / 2.0;

    // Root panel.
    round_rect(pm, ox, oy, MENU_W, panel_h, 8.0, bg);
    // Header: app version, small + dim.
    font::draw(pm, &menu.version, ox + PAD + 2.0, oy + 8.0, 1.5, txt_dim);
    // Divider under header.
    if let Some(rect) = Rect::from_xywh(ox + PAD, oy + HEADER_H, MENU_W - PAD * 2.0, 1.0) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(66, 68, 80, 255));
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }

    // Root items.
    for (i, item) in menu.items.iter().enumerate() {
        let (ix, iy, iw, ih) = menu.item_rect(i);
        let hot = menu.hover == Some(i);
        if hot {
            round_rect(pm, ix, iy, iw, ih, 4.0, bg_hi);
        }
        let _ = ih;
        font::draw(pm, &item.label, ix + 6.0, iy + text_dy, fs, txt);
        if item.has_submenu() {
            font::draw(pm, ">", ix + iw - 12.0, iy + text_dy, fs, if hot { txt } else { txt_dim });
        }
    }

    // Expanded submenu, if any.
    if let Some(root) = menu.open_sub {
        let sub = &menu.items[root].submenu;
        let (sx, sy) = menu.sub_origin(root);
        let sub_h = PAD * 2.0 + sub.len() as f32 * ITEM_H;
        round_rect(pm, sx, sy, MENU_W, sub_h, 8.0, bg);
        for (si, item) in sub.iter().enumerate() {
            let (ix, iy, _iw, _ih) = menu.sub_item_rect(root, si);
            if menu.sub_hover == Some(si) {
                round_rect(pm, ix, iy, MENU_W - PAD * 2.0, ITEM_H, 4.0, bg_hi);
            }
            font::draw(pm, &item.label, ix + 6.0, iy + text_dy, fs, txt);
        }
    }
}

fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '~' => [0b000, 0b000, 0b011, 0b110, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        '?' => [0b111, 0b001, 0b010, 0b000, 0b010],
        ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
        'A' => [0b111, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'K' => [0b101, 0b110, 0b100, 0b110, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        _ => [0b111, 0b101, 0b101, 0b101, 0b111],
    }
}
