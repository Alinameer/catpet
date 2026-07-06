//! Pixel-cat sprite sheets (CC-BY, "Cats Rework" by antumdeluge & zaphgames on
//! OpenGameArt — see assets/CREDITS.md). Each sheet is a 96x192 image laid out as
//! a 3-column x 4-row grid of 32x48 frames:
//!
//!   row 0: facing UP    (away — back/butt view)  frames 0,1,2
//!   row 1: facing RIGHT (side profile)           frames 0,1,2
//!   row 2: facing DOWN  (toward viewer — face!)  frames 0,1,2
//!   row 3: facing LEFT  (side profile)           frames 0,1,2
//!
//! Within a row, frame 1 is the "neutral" mid-step; 0 and 2 are the two step
//! extremes. We use col 1 as the idle/stand pose and cycle 0->1->2->1 for walks.

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

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Up is a valid pose we may use for a "walk away" idle later
pub enum Facing {
    Up = 0,    // row 0: away (back view)
    Right = 1, // row 1: side profile facing right
    Down = 2,  // row 2: toward viewer (face)
    Left = 3,  // row 3: side profile facing left
}

/// One decoded colour sheet, pre-sliced into individual RGBA frames.
pub struct Sheet {
    /// frames[row * COLS + col]
    frames: Vec<RgbaImage>,
}

impl Sheet {
    fn from_bytes(bytes: &[u8]) -> Sheet {
        let img = image::load_from_memory(bytes)
            .expect("embedded cat sprite failed to decode")
            .to_rgba8();
        let mut frames = Vec::with_capacity((COLS * ROWS) as usize);
        for row in 0..ROWS {
            for col in 0..COLS {
                let sub = image::imageops::crop_imm(
                    &img,
                    col * FRAME_W,
                    row * FRAME_H,
                    FRAME_W,
                    FRAME_H,
                )
                .to_image();
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
}

impl Sprites {
    pub fn load() -> Sprites {
        Sprites {
            orange: Sheet::from_bytes(ORANGE),
            black: Sheet::from_bytes(BLACK),
            brown: Sheet::from_bytes(BROWN),
            white: Sheet::from_bytes(WHITE),
            rick: Sheet::from_bytes(RICK),
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
        match character.to_ascii_lowercase().as_str() {
            "rick" => &self.rick,
            _ => self.sheet_for(color_name),
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
    fn sheet_dispatches_on_character() {
        let s = Sprites::load();
        assert!(std::ptr::eq(s.sheet("rick", "orange"), &s.rick));
        assert!(std::ptr::eq(s.sheet("RICK", "white"), &s.rick));
        assert!(std::ptr::eq(s.sheet("cat", "black"), &s.black));
        assert!(std::ptr::eq(s.sheet("anything-else", "brown"), &s.brown));
    }

    #[test]
    fn rick_frames_have_cat_dimensions() {
        let s = Sprites::load();
        let f = s.sheet("rick", "orange").frame(Facing::Down, 1);
        assert_eq!((f.width(), f.height()), (FRAME_W, FRAME_H));
    }
}
