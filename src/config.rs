use crate::CMD;
use crokey::{key, KeyCombination};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum KeyBindEvent {
    FocusLeft,
    FocusDown,
    FocusUp,
    FocusRight,
    MenuUp,
    MenuDown,
    EnterInsert,
    EnterNormal,
    Quit,
    Activate,
    None,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "columns")]
    pub columns: usize,

    #[serde(default = "scrollbar")]
    pub scrollbar: bool,

    #[serde(default = "min_height")]
    pub min_height: u16,

    #[serde(default = "sorting")]
    pub sorting: bool,

    #[serde(default = "colors")]
    pub colors: Colors,

    #[serde(default = "symbols")]
    pub symbols: Symbols,

    #[serde(default = "mouse")]
    pub mouse: bool,

    #[serde(default)]
    pub key_map: KeyMaps,
}

#[derive(Debug)]
pub struct KeyMaps {
    pub normal: HashMap<KeyCombination, KeyBindEvent>,
    pub insert: HashMap<KeyCombination, KeyBindEvent>,
}

impl Default for KeyMaps {
    fn default() -> Self {
        Self {
            normal: normal_key_map(),
            insert: insert_key_map(),
        }
    }
}

impl<'de> Deserialize<'de> for KeyMaps {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Inspect the outer shape first, so legacy bindings get a migration error.
        let tables = HashMap::<String, serde_json::Value>::deserialize(deserializer)?;
        if tables
            .keys()
            .any(|name| name != "normal" && name != "insert")
        {
            return Err(serde::de::Error::custom(
                "the flat [key_map] format is no longer supported; move bindings to [key_map.normal] and [key_map.insert]; see config.toml.example",
            ));
        }
        let mut maps = Self::default();
        for (name, value) in tables {
            let overrides = serde_json::from_value::<HashMap<String, KeyBindEvent>>(value)
                .map_err(serde::de::Error::custom)?;
            let target = if name == "normal" {
                &mut maps.normal
            } else {
                &mut maps.insert
            };
            let mut parsed = HashMap::new();
            for (key, action) in overrides {
                let combination =
                    if key.chars().count() == 1 && key.chars().next().unwrap().is_uppercase() {
                        KeyCombination::new(
                            KeyCode::Char(key.chars().next().unwrap()),
                            KeyModifiers::SHIFT,
                        )
                    } else {
                        KeyCombination::from_str(&key).map_err(serde::de::Error::custom)?
                    };
                if parsed.insert(combination, action).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate key binding '{key}' in key_map.{name}",
                    )));
                }
            }
            target.extend(parsed);
        }
        Ok(maps)
    }
}

#[derive(Deserialize, Debug)]
pub struct Symbols {
    #[serde(default = "highlight_symbol")]
    pub highlight_symbol: String,

    #[serde(default = "node_closed_symbol")]
    pub node_closed_symbol: String,

    #[serde(default = "node_open_symbol")]
    pub node_open_symbol: String,

    #[serde(default = "node_no_children_symbol")]
    pub node_no_children_symbol: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Colors {
    #[serde(default = "reset")]
    pub bg: Color,

    #[serde(default = "white")]
    pub fg: Color,

    #[serde(default = "white")]
    pub border_fg: Color,

    #[serde(default = "reset")]
    pub border_bg: Color,

    #[serde(default = "reset")]
    pub bg_focused: Color,

    #[serde(default = "white")]
    pub fg_focused: Color,

    #[serde(default = "green")]
    pub border_fg_focused: Color,

    #[serde(default = "reset")]
    pub border_bg_focused: Color,

    #[serde(default = "green")]
    pub bg_highlighted: Color,

    #[serde(default = "black")]
    pub fg_highlighted: Color,
}

impl Default for Symbols {
    fn default() -> Self {
        Self {
            highlight_symbol: highlight_symbol(),
            node_open_symbol: node_open_symbol(),
            node_closed_symbol: node_closed_symbol(),
            node_no_children_symbol: node_no_children_symbol(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sorting: sorting(),
            symbols: symbols(),
            colors: colors(),
            columns: columns(),
            scrollbar: scrollbar(),
            min_height: min_height(),
            mouse: mouse(),
            key_map: KeyMaps::default(),
        }
    }
}

impl Config {
    pub fn new(path: &Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let builder = config::Config::builder();
        let builder = match path {
            Some(path) => builder
                .add_source(config::File::from(path.clone()).format(config::FileFormat::Toml)),
            None => {
                let path = Self::get_default_config_path()?;
                if !path.exists() {
                    log::info!("Config file not found. Using default configuration.");
                    return Ok(Self::default());
                }
                builder.add_source(config::File::from(path).format(config::FileFormat::Toml))
            }
        };

        let config = builder.build()?.try_deserialize::<Config>()?;
        Ok(config)
    }

    fn get_default_config_path() -> Result<PathBuf, Box<dyn Error>> {
        match dirs::config_dir() {
            Some(conf_dir) => Ok(conf_dir.join(format!("{CMD}/config.toml"))),
            None => Err(Box::<dyn Error>::from(
                "Couldn't determine default config directory.",
            )),
        }
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            bg: reset(),
            fg: white(),
            border_fg: white(),
            border_bg: reset(),
            bg_focused: reset(),
            fg_focused: white(),
            border_fg_focused: green(),
            border_bg_focused: reset(),
            bg_highlighted: green(),
            fg_highlighted: black(),
        }
    }
}

fn colors() -> Colors {
    Colors::default()
}

const fn reset() -> Color {
    Color::Reset
}
const fn black() -> Color {
    Color::Black
}

const fn white() -> Color {
    Color::White
}

const fn green() -> Color {
    Color::Green
}

const fn sorting() -> bool {
    false
}

fn symbols() -> Symbols {
    Symbols::default()
}

fn highlight_symbol() -> String {
    String::new()
}

fn node_closed_symbol() -> String {
    String::from(" ⏷ ")
}

fn node_open_symbol() -> String {
    String::from(" ▶ ")
}

fn node_no_children_symbol() -> String {
    String::from(" ")
}

const fn columns() -> usize {
    3
}

const fn min_height() -> u16 {
    4
}

const fn scrollbar() -> bool {
    true
}

const fn mouse() -> bool {
    true
}

fn normal_key_map() -> HashMap<KeyCombination, KeyBindEvent> {
    let mut map = HashMap::new();
    map.insert(key!(h), KeyBindEvent::FocusLeft);
    map.insert(key!(l), KeyBindEvent::FocusRight);
    map.insert(key!(j), KeyBindEvent::FocusDown);
    map.insert(key!(k), KeyBindEvent::FocusUp);
    map.insert(key!(i), KeyBindEvent::EnterInsert);
    map.insert(key!(ctrl - c), KeyBindEvent::Quit);
    map.insert(key!(q), KeyBindEvent::Quit);
    map
}

fn insert_key_map() -> HashMap<KeyCombination, KeyBindEvent> {
    HashMap::from([
        (key!(j), KeyBindEvent::MenuDown),
        (key!(k), KeyBindEvent::MenuUp),
        (key!(enter), KeyBindEvent::Activate),
        (key!(esc), KeyBindEvent::EnterNormal),
        (key!(q), KeyBindEvent::Quit),
        (key!(ctrl - c), KeyBindEvent::Quit),
    ])
}
