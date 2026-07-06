//! Right-click context menu: model, layout, and hit-testing. Rendering lives in
//! render.rs (`draw_menu`), which reads this model. The menu is drawn inside the
//! cat's own window; the window grows to fit while the menu is open.
//!
//! Coordinates here are window-local pixels (origin = top-left of the window).

/// An action a menu item performs when clicked. Leaf items carry an `Action`;
/// parent items carry a `submenu`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    // Direct actions.
    TogglePomodoro,
    StretchNow,
    ShowOff,      // "Show off my PixelPal" — a little wiggle/hop
    TellName,     // speak the user's name
    SetName,      // prompt to set name (stub -> bubble for now)
    ShowName,     // show current name in a bubble
    FixedMessagePin,   // pin the fixed message (stub)
    FixedMessageClear, // clear it
    OpenReminders,     // open the reminders popup (stub -> bubble)
    BreakStretchToggle,
    SetColor(ColorName), // change the cat's fur colour
    SetCharacter(CharacterName), // switch between cat and rick
    Quit,
}

/// The four sprite colours, as a menu-friendly enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorName {
    Orange,
    Black,
    Brown,
    White,
}

impl ColorName {
    pub fn as_str(self) -> &'static str {
        match self {
            ColorName::Orange => "orange",
            ColorName::Black => "black",
            ColorName::Brown => "brown",
            ColorName::White => "white",
        }
    }
}

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

#[derive(Clone)]
pub struct Item {
    pub label: String,
    /// Some(action) => leaf; None => has a submenu.
    pub action: Option<Action>,
    pub submenu: Vec<Item>,
}

impl Item {
    fn leaf(label: &str, action: Action) -> Item {
        Item { label: label.into(), action: Some(action), submenu: Vec::new() }
    }
    fn parent(label: &str, submenu: Vec<Item>) -> Item {
        Item { label: label.into(), action: None, submenu }
    }
    pub fn has_submenu(&self) -> bool {
        !self.submenu.is_empty()
    }
}

/// Layout constants (window-local px). Kept here so render + hit-test agree.
/// Sized for the readable 5x7 font at ~2px scale.
pub const MENU_W: f32 = 232.0;
pub const ITEM_H: f32 = 24.0;
pub const HEADER_H: f32 = 24.0;
pub const PAD: f32 = 7.0;

pub struct Menu {
    pub open: bool,
    /// Top-left of the root menu panel, window-local.
    pub origin: (f32, f32),
    pub items: Vec<Item>,
    /// Index of the hovered root item, if any.
    pub hover: Option<usize>,
    /// If a submenu is expanded, which root index it belongs to.
    pub open_sub: Option<usize>,
    /// Hovered index within the open submenu.
    pub sub_hover: Option<usize>,
    pub version: String,
}

impl Menu {
    pub fn new(version: &str) -> Menu {
        Menu {
            open: false,
            origin: (0.0, 0.0),
            items: build_items(),
            hover: None,
            open_sub: None,
            sub_hover: None,
            version: version.to_string(),
        }
    }

    pub fn open_at(&mut self, x: f32, y: f32) {
        self.open = true;
        self.origin = (x, y);
        self.hover = None;
        self.open_sub = None;
        self.sub_hover = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.open_sub = None;
        self.hover = None;
        self.sub_hover = None;
    }

    /// Total panel height for the root list (header + items).
    pub fn panel_h(&self) -> f32 {
        HEADER_H + self.items.len() as f32 * ITEM_H + PAD * 2.0
    }

    /// Root item rect (window-local): (x, y, w, h).
    pub fn item_rect(&self, idx: usize) -> (f32, f32, f32, f32) {
        let (ox, oy) = self.origin;
        let y = oy + HEADER_H + PAD + idx as f32 * ITEM_H;
        (ox + PAD, y, MENU_W - PAD * 2.0, ITEM_H)
    }

    /// Submenu panel origin for a given root index (opens to the right).
    pub fn sub_origin(&self, idx: usize) -> (f32, f32) {
        let (ox, oy) = self.origin;
        let y = oy + HEADER_H + PAD + idx as f32 * ITEM_H;
        (ox + MENU_W - 2.0, y)
    }

    pub fn sub_item_rect(&self, root_idx: usize, sub_idx: usize) -> (f32, f32, f32, f32) {
        let (sx, sy) = self.sub_origin(root_idx);
        let y = sy + PAD + sub_idx as f32 * ITEM_H;
        (sx + PAD, y, MENU_W - PAD * 2.0, ITEM_H)
    }

    fn hit(rect: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
        x >= rect.0 && x <= rect.0 + rect.2 && y >= rect.1 && y <= rect.1 + rect.3
    }

    /// Update hover state for a cursor at window-local (x,y). Expands a submenu
    /// when a parent item is hovered.
    pub fn on_move(&mut self, x: f32, y: f32) {
        if !self.open {
            return;
        }
        // Submenu items first (they overlay to the right).
        if let Some(root) = self.open_sub {
            self.sub_hover = None;
            for si in 0..self.items[root].submenu.len() {
                if Self::hit(self.sub_item_rect(root, si), x, y) {
                    self.sub_hover = Some(si);
                    return; // stay in submenu
                }
            }
        }
        // Root items.
        self.hover = None;
        for i in 0..self.items.len() {
            if Self::hit(self.item_rect(i), x, y) {
                self.hover = Some(i);
                if self.items[i].has_submenu() {
                    self.open_sub = Some(i);
                    self.sub_hover = None;
                } else {
                    self.open_sub = None;
                }
                return;
            }
        }
    }

    /// A left click at (x,y). Returns Some(action) if an action item was hit.
    /// Clicking empty space closes the menu (returns None and sets open=false).
    pub fn on_click(&mut self, x: f32, y: f32) -> Option<Action> {
        if !self.open {
            return None;
        }
        // Submenu item?
        if let Some(root) = self.open_sub {
            for si in 0..self.items[root].submenu.len() {
                if Self::hit(self.sub_item_rect(root, si), x, y) {
                    let act = self.items[root].submenu[si].action;
                    self.close();
                    return act;
                }
            }
        }
        // Root item?
        for i in 0..self.items.len() {
            if Self::hit(self.item_rect(i), x, y) {
                if self.items[i].has_submenu() {
                    // Toggle the submenu open; don't close the menu.
                    self.open_sub = Some(i);
                    return None;
                }
                let act = self.items[i].action;
                self.close();
                return act;
            }
        }
        // Clicked outside any item -> close.
        self.close();
        None
    }

    /// Bounding size needed to show the menu (for window growth), window-local.
    /// Returns (width, height) measured from origin.
    pub fn bounds_from_origin(&self) -> (f32, f32) {
        let mut w = MENU_W;
        let mut h = self.panel_h();
        if let Some(root) = self.open_sub {
            let sub_h = PAD * 2.0 + self.items[root].submenu.len() as f32 * ITEM_H;
            let (sx, sy) = self.sub_origin(root);
            w = (sx - self.origin.0) + MENU_W;
            h = h.max((sy - self.origin.1) + sub_h);
        }
        (w, h)
    }
}

fn build_items() -> Vec<Item> {
    vec![
        Item::parent(
            "Character",
            vec![
                Item::leaf("Cat", Action::SetCharacter(CharacterName::Cat)),
                Item::leaf("Rick", Action::SetCharacter(CharacterName::Rick)),
            ],
        ),
        Item::parent(
            "Cat color",
            vec![
                Item::leaf("Orange", Action::SetColor(ColorName::Orange)),
                Item::leaf("Black", Action::SetColor(ColorName::Black)),
                Item::leaf("Brown", Action::SetColor(ColorName::Brown)),
                Item::leaf("White", Action::SetColor(ColorName::White)),
            ],
        ),
        Item::parent(
            "Fixed message",
            vec![
                Item::leaf("Pin note", Action::FixedMessagePin),
                Item::leaf("Clear", Action::FixedMessageClear),
            ],
        ),
        Item::parent(
            "Reminders",
            vec![Item::leaf("Open reminders", Action::OpenReminders)],
        ),
        Item::parent(
            "Pomodoro",
            vec![Item::leaf("Start / Stop", Action::TogglePomodoro)],
        ),
        Item::parent(
            "Break Stretch",
            vec![
                Item::leaf("Stretch now", Action::StretchNow),
                Item::leaf("Toggle reminders", Action::BreakStretchToggle),
            ],
        ),
        Item::leaf("Show off", Action::ShowOff),
        Item::leaf("Tell my name", Action::TellName),
        Item::leaf("Set my name", Action::SetName),
        Item::leaf("Show my name", Action::ShowName),
        Item::leaf("Quit", Action::Quit),
    ]
}

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
