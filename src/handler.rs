use crate::{
    app::{App, AppResult, FocusDirection, Mode},
    config::KeyBindEvent,
};
use crokey::KeyCombination;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

/// Handles the key events and updates the state of [`App`].
pub async fn handle_key_events(key: KeyEvent, app: &mut App) -> AppResult<()> {
    let bindings = match app.mode {
        Mode::Normal => &app.config.key_map.normal,
        Mode::Insert => &app.config.key_map.insert,
    };
    let Some(key_bind_event) = bindings.get(&KeyCombination::from(key)).copied() else {
        return Ok(());
    };
    match key_bind_event {
        KeyBindEvent::Quit => {
            app.quit();
        }
        KeyBindEvent::FocusLeft => {
            app.move_focus(FocusDirection::Left);
        }
        KeyBindEvent::FocusRight => {
            app.move_focus(FocusDirection::Right);
        }
        KeyBindEvent::FocusDown => {
            app.move_focus(FocusDirection::Down);
        }
        KeyBindEvent::FocusUp => {
            app.move_focus(FocusDirection::Up);
        }
        KeyBindEvent::EnterInsert => app.enter_insert(),
        KeyBindEvent::EnterNormal => app.mode = Mode::Normal,
        KeyBindEvent::Activate => {
            let ids = app
                .get_focused_tree_state()
                .map(|state| state.selected().to_vec());
            if let Some(ids) = ids {
                app.activate_menu_item(&ids).await?;
            }
        }
        KeyBindEvent::MenuDown => {
            if let Some(mut tree_state) = app.get_focused_tree_state_mut() {
                if !tree_state.key_down() {
                    tree_state.select_first();
                }
            }
        }
        KeyBindEvent::MenuUp => {
            if let Some(mut tree_state) = app.get_focused_tree_state_mut() {
                if !tree_state.key_up() {
                    tree_state.select_last();
                }
            }
        }
        KeyBindEvent::None => {}
    }
    app.validate_insert_mode();
    Ok(())
}

fn get_pos(mouse_event: MouseEvent) -> Position {
    Position::new(mouse_event.column, mouse_event.row)
}

async fn handle_click(mouse_event: MouseEvent, app: &App) -> AppResult<()> {
    let pos = get_pos(mouse_event);
    let ids = app
        .get_focused_tree_state()
        .and_then(|state| state.rendered_at(pos).map(|ids| ids.to_vec()));
    let Some(ids) = ids else {
        return Ok(());
    };
    app.activate_menu_item(&ids).await?;
    Ok(())
}

fn handle_scroll(mouse_event: MouseEvent, app: &mut App) -> Option<()> {
    let pos = get_pos(mouse_event);

    // If mouse is over focused item, scroll its menu
    if let Some(sni_state) = app.get_focused_sni_state() {
        if sni_state.rect.contains(pos) {
            let mut tree_state = app.get_focused_tree_state_mut()?;
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    tree_state.scroll_up(1);
                }
                MouseEventKind::ScrollDown => {
                    tree_state.scroll_down(1);
                }
                _ => {}
            }
            return Some(());
        }
    }

    // Otherwise scroll the tray layout
    match mouse_event.kind {
        MouseEventKind::ScrollUp => {
            app.layout.scroll_offset = app.layout.scroll_offset.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            app.layout.scroll_offset = app.layout.scroll_offset.saturating_add(1);
        }
        _ => {}
    }
    Some(())
}

async fn handle_move(mouse_event: MouseEvent, app: &mut App) -> Option<()> {
    let pos = get_pos(mouse_event);
    if let Some((_, sni_state)) = app.sni_states.get_index_mut(app.focused_sni_index) {
        if sni_state.rect.contains(pos) {
            let mut tree_state = app.get_focused_tree_state_mut()?;
            let rendered = tree_state.rendered_at(pos)?.to_owned();
            tree_state.select(rendered.to_vec());
            return None;
        } else {
            sni_state.set_focused(false);
        }
    }

    if let Some(k) = &app.get_focused_sni_key_by_position(pos) {
        if let Some(state_tree) = app.sni_states.get_mut(k) {
            state_tree.set_focused(true);
            app.focused_sni_index = app.sni_states.get_index_of(k).unwrap_or(0);
        }
    }

    Some(())
}

pub async fn handle_mouse_event(mouse_event: MouseEvent, app: &mut App) -> AppResult<()> {
    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_click(mouse_event, app).await?;
        }
        MouseEventKind::Down(MouseButton::Right) => {}
        MouseEventKind::Down(MouseButton::Middle) => {}
        MouseEventKind::Moved => {
            let _ = handle_move(mouse_event, app).await;
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let _ = handle_scroll(mouse_event, app);
        }
        _ => {}
    }
    app.sync_focus();
    app.validate_insert_mode();
    Ok(())
}
