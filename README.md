# linux-wallpaperengine-layerd

![Screenshot](readme-assets/screenshot.png)

A compatibility layer that allows applications designed for `linux-wallpaperengine` to use `we-layerd` as their native Wallpaper Engine renderer.

It provides CLI compatibility with `linux-wallpaperengine` while delegating the actual wallpaper rendering to `we-layerd`.

## Requirements

- Linux
- Wayland
- [`we-layerd`](https://github.com/Aromatic05/we-layerd)
- Rust toolchain
- Wallpaper Engine Workshop content

The wrapper is primarily intended for use with Linux Wallpaper Engine frontends that expect the `linux-wallpaperengine` CLI.

## Installation

Clone and build the project:

```bash
git clone https://github.com/thecogumeta/linux-wallpaperengine-layerd.git
cd linux-wallpaperengine-layerd

cargo build --release
```

Then install the wrapper somewhere in your `PATH`:

```bash
mkdir -p ~/.local/bin

ln -sf "$PWD/target/release/linux-wallpaperengine" \
    ~/.local/bin/linux-wallpaperengine
```

Make sure `~/.local/bin` comes before `/usr/bin` in your `PATH`.

You can verify which executable is being used with:

```bash
which linux-wallpaperengine
```

If your shell caches executable paths, refresh it:

```bash
rehash
```

## Usage

The wrapper is designed to be used as a drop-in replacement for `linux-wallpaperengine`.

For example:

```bash
linux-wallpaperengine \
    --screen-root eDP-1 \
    --bg 3479030273
```

The wallpaper can also be specified as a Workshop path:

```bash
linux-wallpaperengine \
    --screen-root eDP-1 \
    /home/user/.local/share/Steam/steamapps/workshop/content/431960/3479030273
```

The wrapper translates the supported `linux-wallpaperengine` options into a `we-layerd` configuration and delegates the actual rendering to `we-layerd`.

## Frontend Integration

This project is particularly useful with frontends that provide a graphical interface for Wallpaper Engine Workshop content but expect `linux-wallpaperengine` as their backend.

For example, a frontend can continue launching:

```bash
linux-wallpaperengine --bg <wallpaper-id>
```

while this compatibility layer transparently starts and controls:

```text
we-layerd
```

This allows the frontend's Workshop browser, playlists, wallpaper management, and other functionality to be used with `we-layerd`'s native renderer.

## Wallpaper Engine Workshop

Wallpaper Engine Workshop content is expected at:

```text
~/.local/share/Steam/steamapps/workshop/content/431960/
```

The wrapper uses the Workshop item ID to locate the wallpaper automatically.

For example:

```text
431960/3479030273/
```

becomes:

```bash
linux-wallpaperengine --bg 3479030273
```

The Wallpaper Engine assets directory defaults to:

```text
~/.local/share/Steam/steamapps/common/wallpaper_engine/assets
```

This can be overridden with:

```bash
--assets-dir /path/to/assets
```

## Scaling

`linux-wallpaperengine` and `we-layerd` use different names for one of their scaling modes.

The wrapper translates:

| linux-wallpaperengine | we-layerd |
| --------------------- | --------- |
| `fill`                | `cover`   |
| `fit`                 | `fit`     |
| `stretch`             | `stretch` |

`fill` is the default and is automatically converted to `cover`.

## Window State Behavior

The wrapper provides sensible defaults for wallpaper behavior when applications change state:

```toml
[rules]
focused = "mute"
maximized = "keep"
fullscreen = "pause"
```

This means:

- **Focused window** → wallpaper audio is muted
- **Maximized window** → wallpaper keeps running
- **Fullscreen window** → wallpaper pauses

These behaviors can be changed through the compatible command-line options.

### Disable automatic muting

```bash
--noautomute
```

This changes the focused rule from:

```toml
focused = "mute"
```

to:

```toml
focused = "keep"
```

### Disable fullscreen pausing

```bash
--no-fullscreen-pause
```

This changes:

```toml
fullscreen = "pause"
```

to:

```toml
fullscreen = "keep"
```

## Daemon Lifecycle

The compatibility layer starts `we-layerd` automatically when necessary.

The daemon is placed into its own process session using:

```text
setsid()
```

and is given:

```text
PR_SET_PDEATHSIG(SIGTERM)
```

This allows the wrapper to cleanly tie the lifetime of `we-layerd` to the compatibility process.

When the frontend terminates the `linux-wallpaperengine` process, the associated `we-layerd` instance is also terminated.

The wrapper does not rely exclusively on the `we-layerd` control socket to determine whether startup succeeded. The daemon process itself is checked after initialization because the control socket is not guaranteed to appear in every configuration.

## Configuration

The wrapper generates temporary `we-layerd` configuration files under:

```text
$XDG_RUNTIME_DIR/linux-wallpaperengine/
```

For example:

```text
/run/user/1000/linux-wallpaperengine/3479030273.toml
```

These files are generated automatically and do not need to be edited manually.

Wallpaper changes are applied through:

```bash
we-layerd --cli switch --config <config>
```

This allows the frontend to change wallpapers without restarting the renderer unnecessarily.

## Supported Options

The wrapper currently supports the main options expected by `linux-wallpaperengine` frontends:

```text
--screen-root
--bg
--fps
--volume
--silent
--noautomute
--no-audio-processing
--disable-mouse
--disable-parallax
--disable-particles
--no-fullscreen-pause
--scaling
--assets-dir
--set-property
--window
--playlist
```

Some options may have limited or renderer-dependent behavior because the actual rendering is handled by `we-layerd`.

## Why?

`linux-wallpaperengine` provides broad compatibility with existing Linux Wallpaper Engine frontends, while `we-layerd` provides a native renderer designed for Wayland.

This project bridges the two:

```text
┌─────────────────────────────┐
│   Wallpaper Engine Frontend │
│   Workshop / Playlists / UI │
└──────────────┬──────────────┘
               │
               │ linux-wallpaperengine CLI
               ▼
┌─────────────────────────────┐
│ linux-wallpaperengine-layerd│
│     Compatibility Layer     │
└──────────────┬──────────────┘
               │
               │ generated config
               ▼
┌─────────────────────────────┐
│          we-layerd          │
│    Native Wayland Renderer  │
└─────────────────────────────┘
```

The goal is to keep the frontend ecosystem while using `we-layerd` for the actual wallpaper rendering.

## License

See [LICENSE](LICENSE) for the license of this project.
