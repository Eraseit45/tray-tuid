# tray-tuid
Forked from [tray-tui](https://github.com/Levizor/tray-tui)

A **system tray implementation** for **terminal user interfaces (TUI)** using [ratatui](https://github.com/ratatui) and [system-tray](https://github.com/jakestanger/system-tray).

## **Overview**

tray-tui brings system tray functionality to the terminal, displaying **tray menus as interactive trees**. `tray-tuid` collects tray data throughout the desktop session. `tray-tui` connects to that daemon when you open the terminal interface. Closing the interface leaves the daemon running.

## **Features**

✅ **System tray integration** in a terminal
✅ **Interactive tree-based menu navigation**
✅ **Persistent tray collection with an independent TUI client**
✅ **Built using Rust and ratatui**

## **Installation**

Install from crates.io with Cargo:

```sh
cargo install tray-tuid --locked
```

The package installs both commands:

- `tray-tuid`: the daemon that collects tray items and handles menu actions.
- `tray-tuic`: the terminal client that connects to the daemon.

To install from a local checkout:

```sh
cargo install --path . --locked
```

## **Usage**

Start the daemon in one terminal:

```sh
tray-tuid
```

Then open the interface in another terminal:

```sh
tray-tuic
```

The daemon stays in the foreground. Your desktop startup script or service manager can run it in the background. Start it before applications whose tray entries you want to collect.

All frontends connect to the same daemon. The daemon keeps collecting while no frontends are open. Each frontend reads the existing TUI configuration; optionally pass a path with `tray-tui -c "$CONFIG"`.

The default socket is `$XDG_RUNTIME_DIR/tray-tui.sock`. Both programs accept `--socket PATH` to use a different path. They must use the same path. `tray-tui` reports an error if it cannot connect to the daemon. A second daemon cannot bind the same socket.

The frontend and daemon exchange JSON messages over a Unix domain socket. See the [protocol](docs/protocol.md) and [architecture decisions](docs/adr/).

The frontend starts in **Normal** mode. The bottom status line shows the current mode and focused tray item.

| Mode | Key | Action |
| --- | --- | --- |
| Normal | `h/j/k/l` | Move tray focus left/down/up/right |
| Normal | `i` | Enter Insert mode for the focused menu |
| Insert | `j/k` | Select the next/previous menu entry |
| Insert | `Enter` | Activate an entry or expand/collapse a submenu |
| Insert | `Esc` | Return to Normal mode |
| Both | `q` / `Ctrl-C` | Close the frontend |

Each pane remembers its selected menu entry while the frontend is running. Entering Insert selects the first visible actionable entry if the previous selection is unavailable. An empty or unusable menu stays in Normal mode. Removing the focused tray item or its usable menu returns to Normal.

Mouse focus, selection, clicks, and scrolling work in both modes. Moving the mouse to another pane in Insert keeps that mode when the new pane has a usable menu. Arrow keys, Shift navigation, and `h/l` within Insert have no default bindings.

## **Development**

Run `cargo test --locked --all-targets`. Integration tests require `dbus-daemon` and `python3`. They use a private D-Bus session and run the actual frontend through a pseudo-terminal against a controlled Unix socket.

## **Configuration**

Configuration file is located at `$XDG_CONFIG_HOME/tray-tuic/config.toml`.
See [config.toml.example](./config.toml.example) for the complete configuration.

Keyboard bindings use `[key_map.normal]` and `[key_map.insert]`. Each table merges its overrides with that mode's defaults; the same key can have different actions in the two modes. Set an action to `"none"` to disable a default binding. Available actions are `focus_left`, `focus_down`, `focus_up`, `focus_right`, `menu_up`, `menu_down`, `enter_insert`, `enter_normal`, `activate`, `quit`, and `none`.

The old flat `[key_map]` format is no longer supported. Replace it with the two mode tables from the example. Invalid actions, invalid key combinations, and equivalent duplicate bindings within a mode cause a configuration error before the terminal interface starts. Colors, symbols, layout options, and the configuration location retain their existing behavior.

## Showcase

![](images/1.png)

## **License**

Project is licensed under [MIT](./LICENSE) license.
