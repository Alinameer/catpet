# Character Switching (Cat / Rick) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user switch the desktop pet between the cat and a new pixel-art Rick via the right-click menu, persisted across restarts.

**Architecture:** A `character` string in the config (default `"cat"`) selects the sprite sheet at the single sheet-selection call site in `render.rs`. A new Rick sheet is embedded exactly like the cat sheets (96×192 PNG, 3×4 grid of 32×48 frames), so all animation logic works unchanged. Menu, IPC, and CLI each gain a character command mirroring the existing color ones.

**Tech Stack:** Rust 2021 (winit/softbuffer/tiny-skia/image), Python 3 + PIL (one-off sprite generation only, script not committed).

**Spec:** `docs/superpowers/specs/2026-07-06-character-switching-design.md`

## Global Constraints

- **No new crate dependencies.** Config stays hand-rolled `key=value` lines; unknown keys/values are ignored or fall back to defaults.
- **Sheet format is fixed:** 96×192 PNG, 3 cols × 4 rows of 32×48 frames; rows = facing up/right/down/left; column 1 = idle pose, columns 0→1→2→1 = walk cycle (see `src/sprite.rs` header).
- **Character values are the strings `"cat"` and `"rick"`** (matched case-insensitively; anything unrecognized behaves as `"cat"`).
- **Rick has one look:** the color setting is ignored while Rick is active but must survive for switching back to cat.
- **Original art only:** `assets/sprites/rick.png` is original fan art, never extracted show assets. Record this in `assets/CREDITS.md`.
- **The eye-tracking patch (`patch_eyes` in `render.rs`) is cat-only.** It writes pixels at cat-specific coordinates and must not run when Rick is active.
- Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- All commands run from the repo root `/home/mus/Documents/project/catpet`.

---

### Task 1: Generate the Rick sprite sheet

**Files:**
- Create: `assets/sprites/rick.png` (committed)
- Modify: `assets/CREDITS.md`
- Scratch (NOT committed): `/tmp/claude-1000/-home-mus-Documents-project-catpet/5065e7e3-702f-46e1-8d69-ea616a11682f/scratchpad/gen_rick.py`

**Interfaces:**
- Produces: `assets/sprites/rick.png` — 96×192 RGBA PNG in the exact cat sheet layout. Task 3 embeds it via `include_bytes!("../assets/sprites/rick.png")`.

- [ ] **Step 1: Write the generator script** to the scratch path above:

```python
#!/usr/bin/env python3
"""Generate assets/sprites/rick.png - an original pixel-art scientist
(Rick homage) in the cat sheet format: 96x192, 3 cols x 4 rows of 32x48
frames. Rows: 0=up, 1=right, 2=down, 3=left. Col 1 = idle, cols 0/2 = steps."""
import sys
from PIL import Image, ImageOps

FRAME_W, FRAME_H, COLS = 32, 48, 3

HAIR    = (154, 208, 224, 255)   # spiky light blue
HAIR_DK = (112, 172, 194, 255)
SKIN    = (237, 220, 176, 255)   # pale yellow-tan
SKIN_DK = (206, 186, 142, 255)
COAT    = (238, 240, 243, 255)   # white lab coat
COAT_DK = (198, 204, 212, 255)
SHIRT   = (108, 178, 214, 255)   # blue shirt
PANTS   = (108, 80, 56, 255)     # brown
SHOE    = (44, 42, 46, 255)
WHITE   = (250, 250, 250, 255)
PUPIL   = (30, 30, 34, 255)
MOUTH   = (146, 106, 100, 255)
OUTLINE = (56, 62, 70, 255)
EMPTY   = (0, 0, 0, 0)

def rect(img, x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if 0 <= x < FRAME_W and 0 <= y < FRAME_H:
                img.putpixel((x, y), c)

def outline(img):
    """1px dark outline on empty pixels touching the figure (readable on any bg)."""
    src = img.copy()
    for y in range(FRAME_H):
        for x in range(FRAME_W):
            if src.getpixel((x, y))[3] != 0:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < FRAME_W and 0 <= ny < FRAME_H:
                    p = src.getpixel((nx, ny))
                    if p[3] != 0 and p != OUTLINE:
                        img.putpixel((x, y), OUTLINE)
                        break
    return img

SPIKE_TIPS = [7, 4, 6, 3, 5, 3, 6, 4, 7]           # y of each hair spike tip
SPIKE_XS   = [10, 11, 12, 14, 15, 17, 18, 20, 21]  # spike columns

def arms_legs_front(img, step):
    """Shared arms/belt/legs for the front and back views. step in {-1,0,1}."""
    la, ra = (1, -1) if step > 0 else (-1, 1) if step < 0 else (0, 0)
    rect(img, 8, 23 + la, 9, 33 + la, COAT)           # arm L
    rect(img, 23, 23 + ra, 24, 33 + ra, COAT)         # arm R
    rect(img, 8, 34 + la, 9, 35 + la, SKIN)           # hand L
    rect(img, 23, 34 + ra, 24, 35 + ra, SKIN)         # hand R
    rect(img, 10, 36, 22, 36, PANTS)                  # belt
    ll, rl = (-1, 0) if step > 0 else (0, -1) if step < 0 else (0, 0)
    rect(img, 12, 37, 15, 44 + ll, PANTS)             # leg L
    rect(img, 17, 37, 20, 44 + rl, PANTS)             # leg R
    rect(img, 11, 45 + ll, 15, 46 + ll, SHOE)         # shoe L
    rect(img, 17, 45 + rl, 21, 46 + rl, SHOE)         # shoe R

def draw_down(step):
    img = Image.new("RGBA", (FRAME_W, FRAME_H), EMPTY)
    for x, ty in zip(SPIKE_XS, SPIKE_TIPS):
        rect(img, x, ty, x + 1, 11, HAIR)             # hair spikes
    rect(img, 9, 9, 23, 12, HAIR)                     # hair base
    rect(img, 9, 12, 10, 17, HAIR)                    # side hair L
    rect(img, 22, 12, 23, 17, HAIR)                   # side hair R
    rect(img, 11, 12, 21, 21, SKIN)                   # face
    rect(img, 12, 14, 20, 14, HAIR_DK)                # unibrow
    rect(img, 12, 15, 14, 16, WHITE)                  # eye L
    rect(img, 18, 15, 20, 16, WHITE)                  # eye R
    img.putpixel((13, 16), PUPIL)
    img.putpixel((19, 16), PUPIL)
    rect(img, 15, 16, 16, 18, SKIN_DK)                # nose
    rect(img, 13, 20, 19, 20, MOUTH)                  # mouth
    rect(img, 10, 22, 22, 35, COAT)                   # coat
    rect(img, 14, 22, 18, 35, SHIRT)                  # shirt strip
    rect(img, 13, 22, 13, 35, COAT_DK)                # lapel L
    rect(img, 19, 22, 19, 35, COAT_DK)                # lapel R
    arms_legs_front(img, step)
    return outline(img)

def draw_up(step):
    img = Image.new("RGBA", (FRAME_W, FRAME_H), EMPTY)
    for x, ty in zip(SPIKE_XS, SPIKE_TIPS):
        rect(img, x, ty, x + 1, 11, HAIR)
    rect(img, 9, 9, 23, 21, HAIR)                     # full back of head
    rect(img, 12, 13, 12, 19, HAIR_DK)                # hair streaks
    rect(img, 16, 12, 16, 20, HAIR_DK)
    rect(img, 20, 13, 20, 19, HAIR_DK)
    rect(img, 10, 22, 22, 35, COAT)                   # coat back
    rect(img, 16, 24, 16, 35, COAT_DK)                # center vent
    arms_legs_front(img, step)
    return outline(img)

def draw_right(step):
    img = Image.new("RGBA", (FRAME_W, FRAME_H), EMPTY)
    for ty, x0 in ((4, 9), (6, 8), (8, 7), (10, 8)):  # spikes swept back (left)
        rect(img, x0, ty, 16, ty + 1, HAIR)
    rect(img, 10, 4, 19, 12, HAIR)                    # hair mass
    rect(img, 10, 12, 12, 19, HAIR)                   # back-of-head hair
    rect(img, 13, 12, 21, 21, SKIN)                   # face profile
    rect(img, 21, 15, 23, 17, SKIN)                   # big nose
    img.putpixel((23, 17), SKIN_DK)
    rect(img, 16, 14, 20, 14, HAIR_DK)                # brow
    rect(img, 18, 15, 20, 16, WHITE)                  # eye
    img.putpixel((20, 16), PUPIL)
    rect(img, 18, 20, 21, 20, MOUTH)
    rect(img, 11, 22, 20, 35, COAT)                   # torso
    rect(img, 19, 22, 20, 35, SHIRT)                  # shirt sliver at front
    rect(img, 11, 36, 20, 36, PANTS)                  # belt
    if step == 0:
        rect(img, 12, 37, 15, 44, PANTS)              # far leg
        rect(img, 16, 37, 19, 44, PANTS)              # near leg
        rect(img, 12, 45, 17, 46, SHOE)
        rect(img, 16, 45, 21, 46, SHOE)
    else:
        f = 2 * step
        rect(img, 13 - f, 37, 16 - f, 44, PANTS)      # far leg (drawn first)
        rect(img, 12 - f, 45, 17 - f, 46, SHOE)
        rect(img, 15 + f, 37, 18 + f, 44, PANTS)      # near leg
        rect(img, 15 + f, 45, 20 + f, 46, SHOE)
    aa = step                                          # near arm swings
    rect(img, 14 + aa, 24, 17 + aa, 33, COAT)
    rect(img, 14 + aa, 24, 14 + aa, 33, COAT_DK)
    rect(img, 15 + aa, 34, 17 + aa, 35, SKIN)         # hand
    return outline(img)

def main(out_path, preview_path):
    sheet = Image.new("RGBA", (FRAME_W * COLS, FRAME_H * 4), EMPTY)
    steps = (-1, 0, 1)                                 # cols 0, 1, 2
    for row, fn in ((0, draw_up), (1, draw_right), (2, draw_down)):
        for col, s in enumerate(steps):
            sheet.paste(fn(s), (col * FRAME_W, row * FRAME_H))
    for col, s in enumerate(steps):                    # row 3 = left = mirrored right
        sheet.paste(ImageOps.mirror(draw_right(s)), (col * FRAME_W, 3 * FRAME_H))
    sheet.save(out_path)
    prev = sheet.resize((sheet.width * 4, sheet.height * 4), Image.NEAREST)
    prev.save(preview_path)
    print(f"wrote {out_path} and {preview_path}")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
```

- [ ] **Step 2: Run it and eyeball the preview**

```bash
SCRATCH=/tmp/claude-1000/-home-mus-Documents-project-catpet/5065e7e3-702f-46e1-8d69-ea616a11682f/scratchpad
python3 $SCRATCH/gen_rick.py assets/sprites/rick.png $SCRATCH/rick_preview.png
```

Expected: `wrote assets/sprites/rick.png and .../rick_preview.png`

Then **Read the preview PNG** (`$SCRATCH/rick_preview.png`) and check with your own eyes:
- Row 2 (third row, front view): spiky light-blue hair, unibrow over two eyes, white coat with blue shirt strip, brown legs, black shoes.
- Row 1 (right profile): visible big nose pointing right, hair swept back.
- Row 3 (left profile): mirror of row 1.
- Row 0 (back): all hair, no face.
- Columns 0 and 2 differ from column 1 (legs/arms shifted — walk poses).

If something reads wrong (e.g. face illegible, proportions off), adjust the draw functions' rect coordinates and regenerate until it clearly reads as a spiky-haired scientist. Iterate here — this is the only subjective step in the plan.

- [ ] **Step 3: Verify sheet dimensions**

```bash
python3 -c "from PIL import Image; i = Image.open('assets/sprites/rick.png'); print(i.size, i.mode)"
```

Expected: `(96, 192) RGBA`

- [ ] **Step 4: Add credits note** — in `assets/CREDITS.md`, insert before the final paragraph ("Everything else in CatPet..."):

```markdown
The scientist sprite (`rick.png`) is **original pixel art created for this
project** — a fan homage, not extracted or traced from any show asset.
```

- [ ] **Step 5: Commit**

```bash
git add assets/sprites/rick.png assets/CREDITS.md
git commit -m "Add original pixel-art Rick sprite sheet

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Config gains a `character` field

**Files:**
- Modify: `src/config.rs` (whole file is 59 lines; changes shown below)

**Interfaces:**
- Produces: `Config.character: String` (default `"cat"`), parsed from a `character=` line, written by `save()`. New private method `fn apply(&mut self, text: &str)` used by `load()` and tests.
- Consumed by: Task 5 (`main.rs` handlers set it; `render.rs` reads it).

- [ ] **Step 1: Write the failing tests** — append to `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_character_line() {
        let mut cfg = Config::default();
        cfg.apply("color=black\ncharacter=rick\n");
        assert_eq!(cfg.color_name, "black");
        assert_eq!(cfg.character, "rick");
    }

    #[test]
    fn missing_character_defaults_to_cat() {
        let mut cfg = Config::default();
        cfg.apply("color=white\n");
        assert_eq!(cfg.character, "cat");
    }

    #[test]
    fn legacy_and_junk_lines_ignored() {
        let mut cfg = Config::default();
        cfg.apply("pattern=tabby\ngarbage\ncharacter=\n");
        assert_eq!(cfg.character, "cat"); // empty value ignored
        assert_eq!(cfg.color_name, "orange");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin catpet config`
Expected: compile error — `no field `character`` / `no method named `apply``

- [ ] **Step 3: Implement** — in `src/config.rs`:

Add the field (struct + Default):

```rust
#[derive(Clone, Debug)]
pub struct Config {
    /// One of the sprite colours: orange | black | brown | white.
    pub color_name: String,
    /// Active character: "cat" | "rick". Unknown values behave as "cat".
    pub character: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            color_name: "orange".into(),
            character: "cat".into(),
        }
    }
}
```

Replace the body of `load()` and add `apply()`:

```rust
    pub fn load() -> Self {
        let mut cfg = Config::default();
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            cfg.apply(&text);
        }
        cfg
    }

    /// Parse `key=value` lines into self. Unknown keys (incl. legacy
    /// `pattern=`) and empty values are ignored.
    fn apply(&mut self, text: &str) {
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "color" if !v.is_empty() => self.color_name = v.to_string(),
                "character" if !v.is_empty() => self.character = v.to_string(),
                _ => {}
            }
        }
    }
```

Update `save()`'s body line:

```rust
        let body = format!("color={}\ncharacter={}\n", self.color_name, self.character);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin catpet config`
Expected: `test result: ok. 3 passed` (module-filtered)

Note: `cargo test` also compiles the rest of the bin — `main.rs` does not reference `character` yet, and `Config` is constructed via `Default`/`load()` everywhere (`Config::default()` in `dump_frames`), so nothing else breaks.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "Persist active character in config (character=cat|rick)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Sprites embed the Rick sheet and dispatch on character

**Files:**
- Modify: `src/sprite.rs`
- Depends on: Task 1 (`assets/sprites/rick.png` must exist for `include_bytes!`).

**Interfaces:**
- Produces: `Sprites::sheet(&self, character: &str, color_name: &str) -> &Sheet` — `"rick"` (case-insensitive) → Rick sheet, anything else → existing `sheet_for(color_name)`.
- Consumed by: Task 5 (`render.rs` calls `sprites.sheet(&cfg.character, &cfg.color_name)`).

- [ ] **Step 1: Write the failing tests** — append to `src/sprite.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin catpet sprite`
Expected: compile error — `no field `rick`` / `no method named `sheet``

- [ ] **Step 3: Implement** — in `src/sprite.rs`:

After the `WHITE` const (line ~24):

```rust
const RICK: &[u8] = include_bytes!("../assets/sprites/rick.png");
```

Add the field to `Sprites` and its loader:

```rust
pub struct Sprites {
    orange: Sheet,
    black: Sheet,
    brown: Sheet,
    white: Sheet,
    rick: Sheet,
}
```

```rust
    pub fn load() -> Sprites {
        Sprites {
            orange: Sheet::from_bytes(ORANGE),
            black: Sheet::from_bytes(BLACK),
            brown: Sheet::from_bytes(BROWN),
            white: Sheet::from_bytes(WHITE),
            rick: Sheet::from_bytes(RICK),
        }
    }
```

Add the dispatch method to `impl Sprites` (below `sheet_for`):

```rust
    /// Pick the sheet for the active character. Rick has a single look, so
    /// the colour only matters for the cat. Unknown characters act as "cat".
    pub fn sheet(&self, character: &str, color_name: &str) -> &Sheet {
        match character.to_ascii_lowercase().as_str() {
            "rick" => &self.rick,
            _ => self.sheet_for(color_name),
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin catpet sprite`
Expected: `test result: ok. 2 passed` (plus the config tests when unfiltered)

- [ ] **Step 5: Commit**

```bash
git add src/sprite.rs
git commit -m "Embed Rick sprite sheet and add character-aware sheet lookup

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Menu gets a Character submenu

**Files:**
- Modify: `src/menu.rs` (`Action` enum lines 10–24, new enum after `ColorName` ~line 44, `build_items()` lines 225–264)

**Interfaces:**
- Produces: `Action::SetCharacter(CharacterName)`; `CharacterName { Cat, Rick }` with `as_str() -> &'static str` returning `"cat"` / `"rick"`; "Character" is `build_items()[0]` with leaves Cat, Rick.
- Consumed by: Task 5 (`main.rs` matches `Action::SetCharacter`).

- [ ] **Step 1: Write the failing test** — append to `src/menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_submenu_is_first_with_cat_and_rick() {
        let items = build_items();
        assert_eq!(items[0].label, "Character");
        let subs = &items[0].submenu;
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].action, Some(Action::SetCharacter(CharacterName::Cat)));
        assert_eq!(subs[1].action, Some(Action::SetCharacter(CharacterName::Rick)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin catpet menu`
Expected: compile error — `no variant named `SetCharacter`` / `cannot find type `CharacterName``

- [ ] **Step 3: Implement** — in `src/menu.rs`:

Add to the `Action` enum (after `SetColor(ColorName)`, before `Quit`):

```rust
    SetCharacter(CharacterName), // switch between cat and rick
```

Add after the `impl ColorName` block (~line 44):

```rust
/// The available characters, as a menu-friendly enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterName {
    Cat,
    Rick,
}

impl CharacterName {
    pub fn as_str(self) -> &'static str {
        match self {
            CharacterName::Cat => "cat",
            CharacterName::Rick => "rick",
        }
    }
}
```

Add as the FIRST entry of the `vec![...]` in `build_items()` (before "Cat color"):

```rust
        Item::parent(
            "Character",
            vec![
                Item::leaf("Cat", Action::SetCharacter(CharacterName::Cat)),
                Item::leaf("Rick", Action::SetCharacter(CharacterName::Rick)),
            ],
        ),
```

- [ ] **Step 4: Run test — expect a deliberate compile failure in main.rs**

Run: `cargo test --bin catpet menu`
Expected: **compile error in `main.rs`** — `match` on `Action` is now non-exhaustive (`Action::SetCharacter` not covered, ~line 401's match). This is correct and is fixed in Task 5. To confirm the menu code itself is right, temporarily verify with:

Run: `cargo check 2>&1 | grep "not covered"`
Expected: exactly one distinct error, naming `Action::SetCharacter(_)` in `main.rs` (no other files)

- [ ] **Step 5: Commit** (the tree does not compile until Task 5; commit menu.rs only if working solo-inline, otherwise fold this commit into Task 5's. Default: DO commit here, Task 5 immediately follows.)

```bash
git add src/menu.rs
git commit -m "Add Character submenu (Cat / Rick) to right-click menu

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Wire it up — action handler, IPC, CLI, render

**Files:**
- Modify: `src/main.rs` (CLI arm line ~54, help text ~195, IPC handler ~271, action handler ~401, `dump_frames` after the colors loop ~line 119)
- Modify: `src/ipc.rs` (doc comment ~line 10, `Command` enum ~28, `parse()` ~71)
- Modify: `src/render.rs` (`blit_sprite` lines 147 and 152)

**Interfaces:**
- Consumes: `cfg.character` (Task 2), `sprites.sheet(character, color)` (Task 3), `Action::SetCharacter(CharacterName)` (Task 4).
- Produces: `ipc::Command::SetCharacter(String)`; CLI `catpet character <cat|rick>`; `catpet --dump` also emits `rick_idle.png` / `rick_typing.png`.

- [ ] **Step 1: Write the failing IPC parse test** — append to `src/ipc.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_character_command() {
        assert!(matches!(
            parse("character rick"),
            Some(Command::SetCharacter(n)) if n == "rick"
        ));
        assert!(parse("character").is_none()); // arg required
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin catpet ipc`
Expected: compile error — `no variant named `SetCharacter`` (plus the known non-exhaustive-match error from Task 4)

- [ ] **Step 3: Implement all wiring**

`src/ipc.rs` — add to `Command` enum after `SetColor(String)`:

```rust
    SetCharacter(String),
```

Add to `parse()` match after the `"color"` arm:

```rust
        "character" => arg.map(Command::SetCharacter),
```

Update the module doc comment command list (after the `color <name>` line):

```
//!   character <name> -> set character: cat | rick
```

`src/main.rs` — CLI passthrough (line ~54): change

```rust
            "color" | "pattern" => {
```

to

```rust
            "color" | "pattern" | "character" => {
```

`src/main.rs` — help text in `print_help()` (after the `color` line):

```
         catpet character <c>   cat | rick\n\
```

`src/main.rs` — IPC handler: after the `IpcCmd::SetColor(name)` arm (~line 276) add:

```rust
            IpcCmd::SetCharacter(name) => {
                // "cat" | "rick"; sheet() treats unknown values as cat.
                self.cfg.character = name;
                self.cfg.save();
            }
```

`src/main.rs` — menu action handler: after the `Action::SetColor(c)` arm (~line 406) add:

```rust
            Action::SetCharacter(c) => {
                self.cfg.character = c.as_str().to_string();
                self.cfg.save();
                self.state
                    .show_bubble(c.as_str(), Duration::from_millis(1400), now);
            }
```

`src/main.rs` — `dump_frames`: after the closing brace of the `for color in colors` loop (~line 119) add:

```rust
    // Rick character samples.
    {
        let mut st = CatState::new(now);
        st.clock = 0.5;
        st.set_look(0.9, 0.5);
        let mut cfg = Config::default();
        cfg.character = "rick".into();
        let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
        let _ = pm.save_png(&format!("{dir}/rick_idle.png"));
        st.set_mood(Mood::Typing, Duration::from_secs(5), now);
        let pm = render::render(&st, &timers, &cfg, &sprites, &menu, w, w, now);
        let _ = pm.save_png(&format!("{dir}/rick_typing.png"));
    }
```

`src/render.rs` — in `blit_sprite`, line 147: change

```rust
    let sheet = sprites.sheet_for(&cfg.color_name);
```

to

```rust
    let sheet = sprites.sheet(&cfg.character, &cfg.color_name);
```

and line 152 (the eye-patch guard — `patch_eyes` writes cat-specific pixel coords and must not touch Rick):

```rust
    let patched = if matches!(facing, Facing::Down) && !cfg.character.eq_ignore_ascii_case("rick") {
```

- [ ] **Step 4: Run the full test suite and build**

Run: `cargo test`
Expected: `test result: ok.` — 8 tests passing (3 config, 2 sprite, 1 menu, 1 ipc, plus any pre-existing; zero failures), no warnings about non-exhaustive matches.

Run: `cargo build --release`
Expected: `Finished` with no errors.

- [ ] **Step 5: Headless render check**

```bash
./target/release/catpet --dump /tmp/claude-1000/-home-mus-Documents-project-catpet/5065e7e3-702f-46e1-8d69-ea616a11682f/scratchpad/dump
```

**Read** `rick_idle.png` and `rick_typing.png` from that dir. Expected: Rick (not a cat) rendered front-facing; typing frame shows the keyboard under him; no stray "patched eye" pixels on his face. Also read `cat_orange_idle.png` to confirm the cat still renders unchanged.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/ipc.rs src/render.rs
git commit -m "Wire character switching through menu, IPC, CLI and renderer

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: End-to-end verification on the live app

**Files:** none (verification only)

- [ ] **Step 1: Restart the pet with the new binary**

```bash
pkill -f target/release/catpet; sleep 1
nohup ./target/release/catpet >/dev/null 2>&1 &
sleep 2; wmctrl -lp | grep catpet
```

Expected: one window line, e.g. `0x... <desktop> <pid> ... catpet`.

- [ ] **Step 2: Switch via IPC and screenshot**

```bash
./target/release/catpet character rick
sleep 1
WID=$(wmctrl -lp | awk '/catpet/{print $1}')
import -window "$WID" /tmp/claude-1000/-home-mus-Documents-project-catpet/5065e7e3-702f-46e1-8d69-ea616a11682f/scratchpad/e2e_rick.png
```

**Read** the screenshot. Expected: Rick on screen instead of the cat.

- [ ] **Step 3: Persistence check**

```bash
grep character ~/.config/catpet/config.txt
pkill -f target/release/catpet; sleep 1
nohup ./target/release/catpet >/dev/null 2>&1 &
sleep 2
WID=$(wmctrl -lp | awk '/catpet/{print $1}')
import -window "$WID" /tmp/claude-1000/-home-mus-Documents-project-catpet/5065e7e3-702f-46e1-8d69-ea616a11682f/scratchpad/e2e_rick_restart.png
```

Expected: `character=rick` in the config; screenshot after restart still shows Rick.

- [ ] **Step 4: Menu path check (the actual user-facing flow)**

The menu opens at the right-click position; root item 0 ("Character") is at `origin + (PAD..MENU_W-PAD, HEADER_H+PAD .. +ITEM_H)` = center ≈ `(+116, +43)`; its submenu opens at `origin + (MENU_W-2, HEADER_H+PAD)` and sub-item centers are ≈ `(+230+116, +43)` for Cat and `(+230+116, +67)` for Rick (ITEM_H=24, HEADER_H=24, PAD=7, MENU_W=232).

```bash
WID=$(wmctrl -lp | awk '/catpet/{print $1}')
eval $(xdotool getwindowgeometry --shell "$WID")   # sets X, Y, WIDTH, HEIGHT
xdotool mousemove $((X + 60)) $((Y + 120)) click 3          # right-click the pet
sleep 1
xdotool mousemove $((X + 60 + 116)) $((Y + 120 + 43))        # hover "Character"
sleep 1
xdotool mousemove $((X + 60 + 230 + 116)) $((Y + 120 + 43)) click 1   # click "Cat"
sleep 1
import -window "$WID" /tmp/claude-1000/-home-mus-Documents-project-catpet/5065e7e3-702f-46e1-8d69-ea616a11682f/scratchpad/e2e_back_to_cat.png
grep . ~/.config/catpet/config.txt
```

Expected: screenshot shows the cat again (in the previously chosen color) and config reads `character=cat` with the old `color=` intact. If the xdotool coordinates miss (window manager offsets vary), take a screenshot right after the right-click to see where the menu actually drew, adjust the offsets, and retry — or fall back to asking the user to click Character → Cat by hand and confirm.

- [ ] **Step 5: Done — report**

Summarize results with the screenshots. If any check failed, fix before claiming completion (see superpowers:verification-before-completion).

---

## Self-Review Notes

- **Spec coverage:** asset (Task 1), config (Task 2), sprite (Task 3), menu (Task 4), main/ipc/render wiring (Task 5), verification incl. persistence and color-survival (Task 6). The `patch_eyes` guard and `--dump` additions are small extensions beyond the spec text, both serving the spec's "Rick renders correctly" requirement.
- **Type consistency:** `CharacterName::as_str()` returns `"cat"`/`"rick"`; config stores those strings; `Sprites::sheet` matches `"rick"` case-insensitively; unknown → cat everywhere.
- **Compile-order caveat:** the tree intentionally fails to compile between Task 4 Step 3 and Task 5 Step 3 (non-exhaustive match). Task 5 must directly follow Task 4.
