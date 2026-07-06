//! Pixel-cat sprite sheets (CC-BY, "Cats Rework" by antumdeluge & zaphgames on
//! OpenGameArt — see assets/CREDITS.md). Each sheet is a 3-column x 4-row grid;
//! frame size is sheet-relative (cat: 96x192 -> 32x48 frames, rick: 384x768 ->
//! 128x192 frames):
//!
//!   row 0: facing UP    (away — back/butt view)  frames 0,1,2
//!   row 1: facing RIGHT (side profile)           frames 0,1,2
//!   row 2: facing DOWN  (toward viewer — face!)  frames 0,1,2
//!   row 3: facing LEFT  (side profile)           frames 0,1,2
//!
//! Within a row, frame 1 is the "neutral" mid-step; 0 and 2 are the two step
//! extremes. We use col 1 as the idle/stand pose and cycle 0->1->2->1 for walks.
//!
//! Rick additionally ships two standalone one-frame poses (see [`RickPose`]) that
//! replace the whole grid frame for specific interactions: hanging from a hook
//! while dragged, and hunched over a keyboard while typing.

use image::RgbaImage;

pub const FRAME_W: u32 = 32;
pub const FRAME_H: u32 = 48;
pub const COLS: u32 = 3;
pub const ROWS: u32 = 4;

/// The four sheets are embedded so the binary stays a single portable file.
const ORANGE: &[u8] = include_bytes!("../assets/sprites/cat_orange.png");
const BLACK: &[u8] = include_bytes!("../assets/sprites/cat_black.png");
const BROWN: &[u8] = include_bytes!("../assets/sprites/cat_brown.png");
const WHITE: &[u8] = include_bytes!("../assets/sprites/cat_white.png");
const RICK: &[u8] = include_bytes!("../assets/sprites/rick.png");
/// Rick's two special one-off poses (not part of the 3x4 grid): reaching up to a
/// hanging hook while dragged, and hunched over a keyboard while typing. Each is
/// a single frame padded to Rick's 2:3 frame aspect so the renderer's box-fit
/// keeps proportions intact.
const RICK_DRAG: &[u8] = include_bytes!("../assets/sprites/rick_drag.png");
const RICK_TYPE: &[u8] = include_bytes!("../assets/sprites/rick_type.png");

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Up is a valid pose we may use for a "walk away" idle later
pub enum Facing {
    Up = 0,    // row 0: away (back view)
    Right = 1, // row 1: side profile facing right
    Down = 2,  // row 2: toward viewer (face)
    Left = 3,  // row 3: side profile facing left
}

/// Which character a config string resolves to. Centralized so the sheet
/// lookup and every render-side special case agree; add new characters here
/// and the compiler will point at every site that needs a decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterKind {
    Cat,
    Rick,
}

/// A special full-body pose Rick strikes for a specific interaction. These live
/// outside the 3x4 walk grid because they replace the whole figure (arm raised to
/// a hook, hunched over a keyboard) rather than a step in a cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RickPose {
    /// Reaching up, hanging from a ceiling hook — shown while being dragged.
    Drag,
    /// Hunched over a keyboard, hands on the keys — shown while the user types.
    Type,
}

/// Resolve a config character string. Unknown values act as the cat.
pub fn kind(character: &str) -> CharacterKind {
    if character.eq_ignore_ascii_case("rick") {
        CharacterKind::Rick
    } else {
        CharacterKind::Cat
    }
}

/// Decode a single embedded PNG to RGBA. Used for Rick's one-off pose frames,
/// which aren't sliced into a grid.
fn decode(bytes: &[u8]) -> RgbaImage {
    image::load_from_memory(bytes)
        .expect("embedded sprite failed to decode")
        .to_rgba8()
}

/// Horizontal centre of a frame's feet, as a fraction of frame width (0..1).
///
/// The art isn't always centred in its frame (the standing Rick sits in the
/// right two-thirds of his cell), so the ground shadow uses this to sit under
/// the actual feet rather than the frame centre. "Feet" = the bottom ~6% of the
/// opaque figure. Falls back to 0.5 for an empty frame.
pub fn feet_center_frac(img: &RgbaImage) -> f32 {
    let (w, h) = (img.width(), img.height());
    // Bottom-most opaque row.
    let mut bot = None;
    'outer: for y in (0..h).rev() {
        for x in 0..w {
            if img.get_pixel(x, y).0[3] > 8 {
                bot = Some(y);
                break 'outer;
            }
        }
    }
    let Some(bot) = bot else { return 0.5 };
    let span = ((h as f32 * 0.06) as u32).max(1);
    let top = bot.saturating_sub(span);
    let (mut sum, mut n) = (0u64, 0u64);
    for y in top..=bot {
        for x in 0..w {
            if img.get_pixel(x, y).0[3] > 8 {
                sum += x as u64;
                n += 1;
            }
        }
    }
    if n == 0 {
        0.5
    } else {
        (sum as f32 / n as f32) / w as f32
    }
}

/// Split a horizontal `frames`-wide animation strip into equal-width frames.
fn slice_strip(img: &RgbaImage, frames: u32) -> Vec<RgbaImage> {
    let fw = img.width() / frames;
    let fh = img.height();
    (0..frames)
        .map(|i| image::imageops::crop_imm(img, i * fw, 0, fw, fh).to_image())
        .collect()
}

/// One decoded colour sheet, pre-sliced into individual RGBA frames.
pub struct Sheet {
    /// frames[row * COLS + col]
    frames: Vec<RgbaImage>,
}

impl Sheet {
    fn from_bytes(bytes: &[u8]) -> Sheet {
        let img = image::load_from_memory(bytes)
            .expect("embedded sprite sheet failed to decode")
            .to_rgba8();
        // The grid is always 3x4, but the frame size comes from the sheet
        // itself so characters can ship at different resolutions (cat 32x48,
        // rick 128x192). The renderer scales every frame into the same
        // FRAME_W x FRAME_H * SCALE on-screen box.
        let fw = img.width() / COLS;
        let fh = img.height() / ROWS;
        let mut frames = Vec::with_capacity((COLS * ROWS) as usize);
        for row in 0..ROWS {
            for col in 0..COLS {
                let sub = image::imageops::crop_imm(&img, col * fw, row * fh, fw, fh).to_image();
                frames.push(sub);
            }
        }
        Sheet { frames }
    }

    pub fn frame(&self, facing: Facing, col: u32) -> &RgbaImage {
        let col = col % COLS;
        let idx = (facing as u32) * COLS + col;
        &self.frames[idx as usize]
    }
}

/// All colours, decoded once at startup.
pub struct Sprites {
    orange: Sheet,
    black: Sheet,
    brown: Sheet,
    white: Sheet,
    rick: Sheet,
    /// Drag pose is a single still frame.
    rick_drag: RgbaImage,
    /// Type pose is a horizontal strip of `RICK_TYPE_FRAMES` frames the renderer
    /// cycles so Rick's hands actually tap the keyboard.
    rick_type: Vec<RgbaImage>,
}

/// The typing pose ships as a horizontal strip of this many equal-width frames.
pub const RICK_TYPE_FRAMES: u32 = 4;

impl Sprites {
    pub fn load() -> Sprites {
        Sprites {
            orange: Sheet::from_bytes(ORANGE),
            black: Sheet::from_bytes(BLACK),
            brown: Sheet::from_bytes(BROWN),
            white: Sheet::from_bytes(WHITE),
            rick: Sheet::from_bytes(RICK),
            rick_drag: decode(RICK_DRAG),
            rick_type: slice_strip(&decode(RICK_TYPE), RICK_TYPE_FRAMES),
        }
    }

    /// A frame of one of Rick's special full-body poses. `col` selects the
    /// animation frame (only the typing pose has more than one; drag ignores it).
    /// Only meaningful when the active character is Rick.
    pub fn rick_pose(&self, pose: RickPose, col: u32) -> &RgbaImage {
        match pose {
            RickPose::Drag => &self.rick_drag,
            RickPose::Type => {
                let i = (col as usize) % self.rick_type.len();
                &self.rick_type[i]
            }
        }
    }

    /// Map a config colour name to the nearest available sheet. The pack only
    /// ships 4 colours, so grey/blue/pink fall back to sensible neighbours.
    pub fn sheet_for(&self, color_name: &str) -> &Sheet {
        match color_name.to_ascii_lowercase().as_str() {
            "orange" | "ginger" | "pink" => &self.orange,
            "black" => &self.black,
            "brown" | "choco" => &self.brown,
            "white" | "cream" | "grey" | "gray" | "blue" => &self.white,
            _ => &self.orange,
        }
    }

    /// Pick the sheet for the active character. Rick has a single look, so
    /// the colour only matters for the cat. Unknown characters act as "cat".
    pub fn sheet(&self, character: &str, color_name: &str) -> &Sheet {
        match kind(character) {
            CharacterKind::Rick => &self.rick,
            CharacterKind::Cat => self.sheet_for(color_name),
        }
    }

    #[allow(dead_code)] // used by external callers / future CLI listing
    pub fn color_names() -> &'static [&'static str] {
        &["orange", "black", "brown", "white"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_resolves_character_strings() {
        assert_eq!(kind("rick"), CharacterKind::Rick);
        assert_eq!(kind("RICK"), CharacterKind::Rick);
        assert_eq!(kind("cat"), CharacterKind::Cat);
        assert_eq!(kind("anything-else"), CharacterKind::Cat);
    }

    #[test]
    fn sheet_dispatches_on_character() {
        let s = Sprites::load();
        assert!(std::ptr::eq(s.sheet("rick", "orange"), &s.rick));
        assert!(std::ptr::eq(s.sheet("RICK", "white"), &s.rick));
        assert!(std::ptr::eq(s.sheet("cat", "black"), &s.black));
        assert!(std::ptr::eq(s.sheet("anything-else", "brown"), &s.brown));
    }

    #[test]
    fn frame_size_comes_from_each_sheet() {
        let s = Sprites::load();
        // Rick ships at 4x the cat's resolution for a crisper render.
        let f = s.sheet("rick", "orange").frame(Facing::Down, 1);
        assert_eq!((f.width(), f.height()), (4 * FRAME_W, 4 * FRAME_H));
        // The cat sheets stay at the native 32x48.
        let c = s.sheet("cat", "orange").frame(Facing::Down, 1);
        assert_eq!((c.width(), c.height()), (FRAME_W, FRAME_H));
    }

    #[test]
    fn rick_poses_are_distinct_and_frame_shaped() {
        let s = Sprites::load();
        let drag = s.rick_pose(RickPose::Drag, 0);
        let typ = s.rick_pose(RickPose::Type, 0);
        // Two different images, not the same handle.
        assert!(!std::ptr::eq(drag, typ));
        // Each is padded to Rick's 2:3 frame aspect so the renderer's box-fit
        // keeps proportions (allow a pixel of rounding slack).
        for pose in [drag, typ] {
            let ratio = pose.width() as f32 / pose.height() as f32;
            let expected = FRAME_W as f32 / FRAME_H as f32;
            assert!((ratio - expected).abs() < 0.02, "aspect {ratio} != {expected}");
        }
    }

    #[test]
    fn typing_pose_has_distinct_animation_frames() {
        let s = Sprites::load();
        // The strip slices into RICK_TYPE_FRAMES frames, col wraps around them,
        // and the hand-bob frames aren't all identical (0 differs from neutral 1).
        let f0 = s.rick_pose(RickPose::Type, 0);
        let f1 = s.rick_pose(RickPose::Type, 1);
        assert!(std::ptr::eq(f0, s.rick_pose(RickPose::Type, RICK_TYPE_FRAMES)));
        assert_ne!(f0.as_raw(), f1.as_raw(), "typing frames should differ");
    }

    /// Opaque content bounding box (min/max y with any non-transparent pixel).
    fn content_y_span(img: &RgbaImage) -> (u32, u32) {
        let (mut top, mut bot) = (img.height(), 0u32);
        for y in 0..img.height() {
            for x in 0..img.width() {
                if img.get_pixel(x, y).0[3] > 8 {
                    top = top.min(y);
                    bot = bot.max(y);
                    break;
                }
            }
        }
        (top, bot)
    }

    #[test]
    fn typing_pose_matches_standing_figure_size() {
        // Regression guard: the typing pose must render at the same size and feet
        // baseline as the standing frame, or Rick visibly grows/shrinks and his
        // shadow detaches when he starts typing. We compare the figure's height
        // fraction and feet position (both as a fraction of frame height).
        let s = Sprites::load();
        let stand = s.sheet("rick", "white").frame(Facing::Down, 1);
        let (st_top, st_bot) = content_y_span(stand);
        let stand_fill = (st_bot - st_top) as f32 / stand.height() as f32;
        let stand_feet = st_bot as f32 / stand.height() as f32;

        let typ = s.rick_pose(RickPose::Type, 1); // neutral frame (both hands down)
        let (t_top, t_bot) = content_y_span(typ);
        let type_fill = (t_bot - t_top) as f32 / typ.height() as f32;
        let type_feet = t_bot as f32 / typ.height() as f32;

        assert!((type_fill - stand_fill).abs() < 0.04, "fill {type_fill} vs {stand_fill}");
        assert!((type_feet - stand_feet).abs() < 0.03, "feet {type_feet} vs {stand_feet}");

        // Horizontal feet alignment: the pose art must sit its feet at the same
        // x-fraction as the standing frame, or Rick (and his shadow) jump
        // sideways when he starts typing. The walk sheet isn't centred in its
        // cell, so the poses were baked to match it rather than the frame centre.
        let stand_feet_x = feet_center_frac(stand);
        let type_feet_x = feet_center_frac(typ);
        assert!(
            (type_feet_x - stand_feet_x).abs() < 0.04,
            "feet_x {type_feet_x} vs {stand_feet_x} — poses would jump sideways vs standing"
        );
    }
}
