# Character Switching (Cat / Rick) — Design

**Date:** 2026-07-06
**Status:** Approved by user

## Goal

Let the user switch the desktop pet between two characters — the existing cat
and a new Rick (Rick and Morty homage) — via the right-click menu. The choice
persists across restarts.

## Background

- Sprites are 96×192 PNG sheets: 3 columns × 4 rows of 32×48 frames.
  Rows are facing up / right / down / left; column 1 is the idle pose and
  columns 0→1→2→1 cycle for walking (see `src/sprite.rs` header comment).
- Exactly one call site picks the active sheet: `render.rs` line ~147,
  `sprites.sheet_for(&cfg.color_name)`.
- The right-click menu already has a "Cat color" submenu with a
  `SetColor(ColorName)` action; character switching mirrors that pattern.
- An IPC command `SetColor` exists; character gets a symmetric one.

## Decision

Introduce a first-class **character** concept (approach B), rejected
alternatives: treating Rick as a fifth "color" (semantically muddy, scales
badly) and a runtime character-pack loader (YAGNI for one extra character).

## Changes

### 1. New asset: `assets/sprites/rick.png`

Original pixel-art homage — spiky light-blue hair, unibrow, white lab coat,
blue shirt, brown pants. Identical sheet format to the cat (96×192, 3×4 grid
of 32×48 frames, same row semantics, transparent background), so all existing
animation logic works unchanged. Generated deterministically by a one-off
script (not committed); iterated against screenshots. `assets/CREDITS.md`
gains a note that it is original fan art, not extracted from the show.

### 2. `src/config.rs`

- New field `character: String`, default `"cat"`.
- Parse a `character=` line in `load()`; write it in `save()`.
- Missing or unrecognized values fall back to `"cat"` — old config files keep
  working untouched.

### 3. `src/sprite.rs`

- Embed `RICK` bytes alongside the four cat sheets; decode into
  `Sprites.rick: Sheet` in `load()`.
- New method `sheet(&self, character: &str, color_name: &str) -> &Sheet`:
  `"rick"` (case-insensitive) → Rick sheet; anything else → existing
  `sheet_for(color_name)`. Rick has a single look; the color setting is
  ignored while Rick is active but still applies when switching back to cat.

### 4. `src/menu.rs`

- New enum `CharacterName { Cat, Rick }` with `as_str()` (mirrors
  `ColorName`).
- New `Action::SetCharacter(CharacterName)`.
- New "Character" parent item at the **top** of `build_items()` with leaves
  "Cat" and "Rick".

### 5. `src/main.rs`

- Handle `Action::SetCharacter(c)`: set `cfg.character`, `cfg.save()` —
  mirrors the `SetColor` handler (~line 401).
- New `IpcCmd::SetCharacter(String)` handled the same way (~line 271),
  mirroring `IpcCmd::SetColor`.

### 6. `src/render.rs`

- Line ~147 becomes `let sheet = sprites.sheet(&cfg.character, &cfg.color_name);`

## Error handling

- Unknown `character=` config value or IPC argument → cat (tolerant string
  match, same philosophy as `sheet_for`'s alias fallbacks).
- Rick sheet decode failure → same `expect()` as the existing embedded
  sheets (can only fail if the committed asset is corrupt; caught at first
  run during development).

## Verification

1. `cargo build --release` compiles clean.
2. Launch; right-click the pet (driven via `xdotool`), click
   Character → Rick; screenshot shows Rick rendering.
3. Observe walk/idle/typing animations on Rick (screenshots).
4. Restart the app; Rick is still the active character (config persisted).
5. Switch back to Cat via the menu; previously chosen fur color is intact.

## Out of scope

- Color/variant support for Rick.
- Runtime-loadable character packs.
- Any changes to animation or behavior logic.
