use std::cell::RefCell;

use ratatui::widgets::{Block, StatefulWidget};
use ratatui::{
    buffer::Buffer,
    layout::{self, Rect},
    style::{Color, Style},
    widgets::Widget,
};
use tray_tui::protocol::{MenuItem, TrayItem, TrayMenu};

use tui_tree_widget::{Tree, TreeItem, TreeState};

use crate::config::Config;

pub type Id = i32;

#[derive(Debug, Default)]
pub struct SniState {
    pub rect: Rect,
    pub focused: bool,
    pub tree_state: RefCell<TreeState<Id>>,
}

impl SniState {
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

/// Wrapper around a tray item and its interface state
#[derive(Debug)]
pub struct Item<'a> {
    pub sni_state: &'a SniState,
    pub item: &'a TrayItem,
    pub menu: &'a Option<TrayMenu>,
    config: &'a Config,
    pub rect: Rect,
}

impl<'a> Item<'a> {
    pub fn new(sni_state: &'a SniState, item: &'a TrayItem, config: &'a Config) -> Self {
        Self {
            sni_state,
            item,
            menu: &item.menu,
            config,
            rect: Rect::default(),
        }
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    pub fn get_colors(&self) -> (Color, Color) {
        let colors = &self.config.colors;
        let mut bg = colors.bg;
        let mut fg = colors.fg;

        if self.sni_state.focused {
            bg = colors.bg_focused;
            fg = colors.fg_focused;
        }

        (bg, fg)
    }

    pub fn get_highlight_colors(&self) -> (Color, Color) {
        let colors = &self.config.colors;
        (colors.bg_highlighted, colors.fg_highlighted)
    }

    pub fn get_border_color(&self) -> (Color, Color) {
        let colors = &self.config.colors;
        match self.sni_state.focused {
            true => (colors.border_bg_focused, colors.border_fg_focused),
            false => (colors.border_bg, colors.border_fg),
        }
    }
}

impl Widget for Item<'_> {
    fn render(self, area: layout::Rect, buf: &mut Buffer) {
        let title = self.item.title.clone();
        let (bg, fg) = self.get_colors();
        let (bg_h, fg_h) = self.get_highlight_colors();
        let (border_bg, border_fg) = self.get_border_color();
        let symbols = &self.config.symbols;

        if let Some(menu) = self.menu {
            let children = menuitems_to_treeitems(&menu.submenus);

            let tree = Tree::new(&children);

            if let Ok(mut tree) = tree {
                tree = tree
                    .style(Style::default().bg(bg).fg(fg))
                    .highlight_style(Style::default().bg(bg_h).fg(fg_h))
                    .highlight_symbol(&symbols.highlight_symbol)
                    .node_open_symbol(&symbols.node_open_symbol)
                    .node_closed_symbol(&symbols.node_closed_symbol)
                    .node_no_children_symbol(&symbols.node_no_children_symbol);
                tree = tree.block(
                    Block::bordered()
                        .title(title)
                        .border_style(Style::default().fg(border_fg).bg(border_bg)),
                );

                StatefulWidget::render(
                    tree,
                    area,
                    buf,
                    &mut self.sni_state.tree_state.borrow_mut(),
                );
            }
        } else {
            let block = Block::default().title(title).style(Style::default());
            block.render(area, buf);
        }
    }
}

fn menuitem_to_treeitem(menu_item: &MenuItem) -> Option<TreeItem<'_, Id>> {
    if !is_rendered(menu_item) {
        return None;
    }
    let id = menu_item.id;
    if menu_item.submenu.is_empty() {
        match &menu_item.label {
            Some(label) => return Some(TreeItem::new_leaf(id, label.clone())),
            None => return None,
        }
    }
    let children = menuitems_to_treeitems(&menu_item.submenu);
    let root = TreeItem::new(
        id,
        menu_item.label.clone().unwrap_or(String::from("no_label")),
        children,
    );

    root.ok()
}

fn menuitems_to_treeitems(menu_items: &[MenuItem]) -> Vec<TreeItem<'_, Id>> {
    menu_items.iter().filter_map(menuitem_to_treeitem).collect()
}

fn is_rendered(item: &MenuItem) -> bool {
    item.visible && (item.label.is_some() || !item.submenu.is_empty())
}

pub trait MenuNavigation {
    fn find_menu_by_id(&self, ids: &[Id]) -> Option<&MenuItem>;
    fn first_actionable(&self) -> Option<Vec<Id>>;
    fn is_actionable(&self, ids: &[Id]) -> bool {
        !ids.is_empty()
            && (1..=ids.len()).all(|length| {
                self.find_menu_by_id(&ids[..length])
                    .is_some_and(|item| item.enabled)
            })
    }
}

impl MenuNavigation for TrayMenu {
    fn find_menu_by_id(&self, ids: &[Id]) -> Option<&MenuItem> {
        let mut items = self.submenus.as_slice();
        let mut result = None;
        for id in ids {
            let item = items
                .iter()
                .find(|item| item.id == *id && is_rendered(item))?;
            result = Some(item);
            items = &item.submenu;
        }
        result
    }

    fn first_actionable(&self) -> Option<Vec<Id>> {
        self.submenus
            .iter()
            .find(|item| is_rendered(item) && item.enabled)
            .map(|item| vec![item.id])
    }
}
