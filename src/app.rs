use indexmap::IndexMap;
use ratatui::layout::{Position, Rect};
use std::{
    cell::{Ref, RefMut},
    error,
};
use tray_tui::{client::Client, protocol::TrayItems};
use tui_tree_widget::TreeState;

use tokio::sync::Mutex;

use crate::wrappers::{Id, MenuNavigation, SniState};
use crate::Config;

pub type BoxStack = Vec<(i32, Rect)>;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
}

#[derive(Debug, Default)]
pub struct Layout {
    pub rows: Vec<Vec<usize>>,
    pub last_col: usize,
    pub scroll_offset: u16,
}

/// Application.
#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub mode: Mode,
    /// Config
    pub config: Config,
    /// Connection to the daemon
    pub client: Mutex<Client>,
    /// Interface states saved for each tray item
    pub sni_states: IndexMap<String, SniState>, // for the StatusNotifierItem
    //  currently focused sni item info
    pub focused_sni_index: usize,
    /// last focused index to detect focus changes for auto-scrolling
    pub last_focused_sni_index: usize,
    pub focused_sni_key: String,
    /// Local view of the daemon's tray state
    pub items: TrayItems,
    pub last_error: Option<String>,
    pub layout: Layout,
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new(client: Client, items: TrayItems, config: Config) -> Self {
        Self {
            running: true,
            mode: Mode::Normal,
            config,
            items,
            last_error: None,
            sni_states: IndexMap::default(),
            client: Mutex::new(client),
            focused_sni_index: 0,
            last_focused_sni_index: 0,
            focused_sni_key: String::default(),
            layout: Layout::default(),
        }
    }

    /// Updating states
    pub fn update(&mut self) {
        // sync key to index
        if let Some(key) = self.get_focused_sni_key() {
            self.focused_sni_key = key.to_owned();
        }

        // create a buffer for items keys and their titles(for sorting)
        let buffer: IndexMap<_, _> = self
            .items
            .iter()
            .map(|(k, v)| (k.to_owned(), v.title.to_owned()))
            .collect();

        // Add sni states if there are in new items
        for (key, _) in &buffer {
            self.sni_states.entry(key.to_owned()).or_default();
        }

        // Remove states that aren't in new items
        self.sni_states.retain(|key, _| buffer.contains_key(key));

        // Sort by titles
        if self.config.sorting {
            self.sni_states
                .sort_by(|k1, _, k2, _| buffer[k1].cmp(&buffer[k2]));
        }

        // sync index to key back
        if let Some(index) = self.sni_states.get_index_of(&self.focused_sni_key) {
            self.focused_sni_index = index;
        } else if !self.sni_states.is_empty() {
            self.mode = Mode::Normal;
            // Key is gone! Reset to a valid neighbor (next or previous)
            self.focused_sni_index = self
                .focused_sni_index
                .min(self.sni_states.len().saturating_sub(1));
            if let Some((k, _)) = self.sni_states.get_index(self.focused_sni_index) {
                self.focused_sni_key = k.clone();
            }
        } else {
            self.mode = Mode::Normal;
            self.focused_sni_index = 0;
            self.focused_sni_key = String::default();
        }

        self.layout.rows = (0..self.sni_states.len())
            .collect::<Vec<_>>()
            .chunks(self.config.columns)
            .map(|chunk| chunk.to_vec())
            .collect();
        // IDs, rather than positions, keep selections stable across menu reorders.
        for (key, state) in &self.sni_states {
            let menu = self.items.get(key).and_then(|item| item.menu.as_ref());
            let mut tree = state.tree_state.borrow_mut();
            let stale: Vec<_> = tree
                .opened()
                .iter()
                .filter(|ids| {
                    menu.and_then(|menu| menu.find_menu_by_id(ids))
                        .is_none_or(|item| item.submenu.is_empty())
                })
                .cloned()
                .collect();
            for ids in stale {
                tree.close(&ids);
            }
            if !menu.is_some_and(|menu| menu.is_actionable(tree.selected())) {
                tree.select(Vec::new());
            }
        }
        // Synchronize focus
        self.sync_focus();
        self.validate_insert_mode();
    }

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn get_items(&self) -> &TrayItems {
        &self.items
    }

    pub fn get_focused_sni_key(&self) -> Option<&String> {
        self.sni_states
            .get_index(*self.get_focused_sni_index())
            .map(|(k, _)| k)
    }

    pub fn get_focused_sni_index(&self) -> &usize {
        &self.focused_sni_index
    }

    pub fn get_focused_sni_state(&self) -> Option<&SniState> {
        let (_, v) = self.sni_states.get_index(self.focused_sni_index)?;
        Some(v)
    }

    pub fn get_focused_sni_state_mut(&mut self) -> Option<&mut SniState> {
        let (_, v) = self.sni_states.get_index_mut(self.focused_sni_index)?;
        Some(v)
    }

    pub fn get_focused_sni_key_by_position(&mut self, pos: Position) -> Option<String> {
        self.sni_states
            .iter()
            .find(|(_, v)| v.rect.contains(pos))
            .map(|(k, _)| k.to_string())
    }

    pub fn get_focused_tree_state(&self) -> Option<Ref<'_, TreeState<Id>>> {
        self.get_focused_sni_state()
            .map(|sni| sni.tree_state.borrow())
    }

    pub fn get_focused_tree_state_mut(&self) -> Option<RefMut<'_, TreeState<Id>>> {
        self.get_focused_sni_state()
            .map(|sni| sni.tree_state.borrow_mut())
    }

    pub fn move_focus(&mut self, direction: FocusDirection) -> Option<()> {
        let total = self.layout.rows.iter().map(|r| r.len()).sum::<usize>();
        if total <= 1 {
            return Some(());
        }

        let index = self.focused_sni_index;
        let cols = self.config.columns;

        let last_row_index = self.layout.rows.len() - 1;
        let last_row_len = self.layout.rows[last_row_index].len();

        let row = index / cols;
        let col = index % cols;

        let new_index = match direction {
            FocusDirection::Left => {
                if index > 0 {
                    index - 1
                } else {
                    total - 1
                }
            }

            FocusDirection::Right => {
                if index + 1 < total {
                    index + 1
                } else {
                    0
                }
            }

            FocusDirection::Up => {
                let target_row = if row == 0 { last_row_index } else { row - 1 };
                let target_len = if target_row == last_row_index {
                    last_row_len
                } else {
                    cols
                };
                let clamped_col = self.layout.last_col.min(target_len - 1);
                target_row * cols + clamped_col
            }

            FocusDirection::Down => {
                let target_row = if row == last_row_index { 0 } else { row + 1 };
                let target_len = if target_row == last_row_index {
                    last_row_len
                } else {
                    cols
                };
                let clamped_col = self.layout.last_col.min(target_len - 1);
                target_row * cols + clamped_col
            }
        };

        match direction {
            FocusDirection::Up | FocusDirection::Down => self.layout.last_col = col,
            _ => self.layout.last_col = new_index % cols,
        }

        if let Some((key, _)) = self.sni_states.get_index(new_index) {
            let key = key.clone();
            self.focused_sni_index = new_index;
            self.focused_sni_key = key;

            if let Some((_, old_state)) = self.sni_states.get_index_mut(index) {
                old_state.set_focused(false);
            }
            self.sync_focus();
        }

        Some(())
    }

    pub fn sync_focus(&mut self) {
        self.focused_sni_key = self.get_focused_sni_key().cloned().unwrap_or_default();
        for (key, state) in &mut self.sni_states {
            state.set_focused(key == &self.focused_sni_key);
        }
    }

    pub fn enter_insert(&mut self) {
        let menu = self
            .items
            .get(&self.focused_sni_key)
            .and_then(|item| item.menu.as_ref());
        if let Some(mut tree) = self.get_focused_tree_state_mut() {
            if !menu.is_some_and(|menu| menu.is_actionable(tree.selected())) {
                tree.select(Vec::new());
            }
        }
        self.mode = Mode::Insert;
        self.validate_insert_mode();
    }

    pub fn validate_insert_mode(&mut self) {
        if self.mode != Mode::Insert {
            return;
        }
        let menu = self
            .items
            .get(&self.focused_sni_key)
            .and_then(|item| item.menu.as_ref());
        let Some((menu, first)) =
            menu.and_then(|menu| menu.first_actionable().map(|first| (menu, first)))
        else {
            self.mode = Mode::Normal;
            return;
        };
        if let Some(mut tree) = self.get_focused_tree_state_mut() {
            if menu.find_menu_by_id(tree.selected()).is_none() {
                tree.select(first);
            }
            let selected = tree.selected().to_vec();
            for length in 1..selected.len() {
                tree.open(selected[..length].to_vec());
            }
        }
    }

    pub async fn activate_menu_item(&self, ids: &[Id]) -> AppResult<()> {
        log::debug!("Entered activate_menu_item");
        let Some(sni_key) = self.get_focused_sni_key() else {
            return Ok(());
        };
        log::debug!("Activating menu item with key: {}", &sni_key);
        let Some(menu) = self.items.get(sni_key).and_then(|item| item.menu.as_ref()) else {
            return Ok(());
        };
        let Some(item) = menu.find_menu_by_id(ids) else {
            return Ok(());
        };
        if !menu.is_actionable(ids) {
            return Ok(());
        }

        if item.submenu.is_empty() {
            self.client
                .lock()
                .await
                .activate(sni_key.to_string(), item.id)
                .await?;
        } else if let Some(mut tree_state) = self.get_focused_tree_state_mut() {
            tree_state.toggle(ids.to_vec());
        }

        Ok(())
    }
}
pub enum FocusDirection {
    Down,
    Up,
    Right,
    Left,
}
