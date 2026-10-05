use anyhow::{Context, Result, bail};
use clap::Parser;
use nix::{
    sys::{prctl::set_pdeathsig, signal::Signal},
    unistd::setsid,
};
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    flag,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const STEAM_APP_ID: &str = "431960";

#[derive(Parser, Debug)]
#[command(
    name = "linux-wallpaperengine",
    about = "Compatibility wrapper for linux-wallpaper-engine using we-layerd"
)]
struct Args {
    #[arg(long = "screen-root")]
    screens: Vec<String>,

    #[arg(long)]
    bg: Option<String>,

    #[arg()]
    background: Option<String>,

    #[arg(long)]
    fps: Option<u32>,

    #[arg(long)]
    volume: Option<f32>,

    #[arg(long)]
    silent: bool,

    #[arg(long = "noautomute")]
    no_automute: bool,

    #[arg(long = "no-audio-processing")]
    no_audio_processing: bool,

    #[arg(long = "disable-mouse")]
    disable_mouse: bool,

    #[arg(long = "disable-parallax")]
    disable_parallax: bool,

    #[arg(long = "disable-particles")]
    disable_particles: bool,

    #[arg(long = "no-fullscreen-pause")]
    no_fullscreen_pause: bool,

    #[arg(long)]
    scaling: Option<String>,

    #[arg(long = "assets-dir")]
    assets_dir: Option<PathBuf>,

    #[arg(long = "set-property", value_parser = parse_property)]
    properties: Vec<(String, String)>,

    #[arg(long)]
    window: Option<String>,

    #[arg(long)]
    playlist: Option<String>,
}

fn parse_property(value: &str) -> Result<(String, String), String> {
    let (name, value) = value
        .split_once('=')
        .ok_or_else(|| "expected NAME=VALUE".to_string())?;

    Ok((name.to_string(), value.to_string()))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let background = args
        .bg
        .or(args.background)
        .context("missing wallpaper background ID/path")?;

    let background_id = resolve_background_id(&background)?;

    let screens = if args.screens.is_empty() {
        detect_screens()?
    } else {
        args.screens
    };

    if screens.is_empty() {
        bail!("no Wayland outputs found");
    }

    let fps = args.fps.unwrap_or(45);
    let volume = args.volume.unwrap_or(0.75);
    let muted = args.silent;

    let scaling = map_scaling(args.scaling.as_deref().unwrap_or("fill"));

    let assets_path = args.assets_dir.unwrap_or_else(default_assets_path);

    let config = build_config(
        &background_id,
        &screens,
        fps,
        volume,
        muted,
        scaling,
        &assets_path,
        args.no_automute,
        args.no_fullscreen_pause,
    );

    let runtime_dir = env::var("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is not set")?;

    let config_dir = PathBuf::from(&runtime_dir).join("linux-wallpaperengine");

    fs::create_dir_all(&config_dir).context("failed to create runtime config directory")?;

    let config_path = config_dir.join(format!("{background_id}.toml"));

    fs::write(&config_path, config)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    ensure_daemon_running(&config_path)?;
    switch_config(&config_path)?;

    /*
     * The UX frontend expects the compatibility process
     * to stay alive while the wallpaper is active.
     */
    let terminated = Arc::new(AtomicBool::new(false));

    flag::register(SIGTERM, Arc::clone(&terminated))
        .context("failed to register SIGTERM handler")?;

    flag::register(SIGINT, Arc::clone(&terminated)).context("failed to register SIGINT handler")?;

    while !terminated.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    /*
     * The daemon has PR_SET_PDEATHSIG, so when this
     * compatibility process dies, we-layerd receives SIGTERM.
     */
    Ok(())
}

fn resolve_background_id(background: &str) -> Result<String> {
    if background.chars().all(|c| c.is_ascii_digit()) {
        return Ok(background.to_string());
    }

    let path = PathBuf::from(background);

    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_owned)
        .context("could not determine Wallpaper Engine workshop ID")
}

fn default_assets_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| String::from("/home/cogumeta"));

    PathBuf::from(home).join(".local/share/Steam/steamapps/common/wallpaper_engine/assets")
}

fn map_scaling(value: &str) -> &'static str {
    match value {
        /*
         * linux-wallpaperengine calls this "fill".
         * we-layerd calls the equivalent "cover".
         */
        "fill" => "cover",
        "fit" => "fit",
        "stretch" => "stretch",
        _ => "cover",
    }
}

fn build_config(
    id: &str,
    screens: &[String],
    fps: u32,
    volume: f32,
    muted: bool,
    scaling: &str,
    assets_path: &Path,
    no_automute: bool,
    no_fullscreen_pause: bool,
) -> String {
    let source =
        format!("/home/cogumeta/.local/share/Steam/steamapps/workshop/content/{STEAM_APP_ID}/{id}");

    /*
     * linux-wallpaperengine:
     *
     * default:
     *   focused -> mute
     *   maximized -> keep
     *   fullscreen -> pause
     *
     * --noautomute:
     *   focused -> keep
     *
     * --no-fullscreen-pause:
     *   fullscreen -> keep
     */
    let focused_rule = if no_automute { "keep" } else { "mute" };

    let fullscreen_rule = if no_fullscreen_pause { "keep" } else { "pause" };

    let mut config = format!(
        r#"[general]
backend = "layer_shell"
interactive = true
force_scene_audio_loop = false
show_fps = false
fps_report_interval_secs = 1
scale_mode = "{scaling}"

[renderer]
library_path = ""
source = "{source}"
assets_path = "{assets_path}"
cache_path = "~/.cache/we-layerd/renderer"
prefer_dmabuf = true
allow_shm_fallback = true
fps = {fps}
max_fps = {fps}
speed = 1.0
volume = {volume}
muted = {muted}
msaa_samples = 1
fill_mode = "{scaling}"

[wallpapers.{id}]
fps = {fps}
speed = 1.0
volume = {volume}
muted = {muted}
fill_mode = "{scaling}"

[integrations]
media = true
audio_spectrum = false
audio_source = "@DEFAULT_MONITOR@"
audio_sample_rate = 48000
audio_update_hz = 30

[rules]
focused = "{focused_rule}"
maximized = "keep"
fullscreen = "{fullscreen_rule}"

"#,
        assets_path = toml_escape_path(assets_path),
        source = toml_escape(&source),
        focused_rule = focused_rule,
        fullscreen_rule = fullscreen_rule,
    );

    for screen in screens {
        config.push_str(&format!(
            r#"[outputs."{}"]
wallpaper_id = "{id}"
source = "{source}"

"#,
            toml_escape(screen),
            source = toml_escape(&source),
        ));
    }

    config
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn toml_escape_path(path: &Path) -> String {
    toml_escape(&path.to_string_lossy())
}

fn daemon_socket() -> PathBuf {
    let runtime_dir =
        env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/run/user/1000"));

    PathBuf::from(runtime_dir)
        .join("we-layerd")
        .join("control.sock")
}

fn daemon_running() -> bool {
    daemon_socket().exists()
}

fn ensure_daemon_running(config_path: &Path) -> Result<()> {
    if daemon_running() {
        return Ok(());
    }

    let mut command = Command::new("we-layerd");

    command
        .args(["--cli", "run", "--config"])
        .arg(config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    /*
     * setsid()
     *   puts we-layerd into its own session/process group.
     *
     * PR_SET_PDEATHSIG(SIGTERM)
     *   makes the kernel terminate we-layerd when this
     *   linux-wallpaperengine wrapper dies.
     */
    unsafe {
        use std::os::unix::process::CommandExt;

        command.pre_exec(|| {
            setsid().map_err(std::io::Error::other)?;

            set_pdeathsig(Some(Signal::SIGTERM)).map_err(std::io::Error::other)?;

            Ok(())
        });
    }

    let mut child = command.spawn().context("failed to start we-layerd")?;

    /*
     * The control socket is not guaranteed to appear.
     * Give the renderer enough time to initialize and
     * only fail if the actual process already exited.
     */
    thread::sleep(Duration::from_millis(500));

    if let Some(status) = child.try_wait().context("failed to check we-layerd")? {
        bail!("we-layerd exited during startup: {status}");
    }

    /*
     * Reap the daemon when it eventually exits.
     */
    thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

fn switch_config(config_path: &Path) -> Result<()> {
    let status = Command::new("we-layerd")
        .args(["--cli", "switch", "--config"])
        .arg(config_path)
        .status()
        .context("failed to execute we-layerd switch")?;

    if !status.success() {
        bail!("we-layerd switch failed with exit status: {status}");
    }

    Ok(())
}

fn detect_screens() -> Result<Vec<String>> {
    let output = Command::new("we-layerd")
        .args(["--cli", "outputs"])
        .output()
        .context("failed to query we-layerd outputs")?;

    if !output.status.success() {
        bail!("we-layerd outputs failed");
    }

    let text = String::from_utf8_lossy(&output.stdout);

    let screens: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();

            if line.is_empty() {
                return None;
            }

            if line.contains(' ')
                || line.contains(':')
                || line.contains('=')
                || line.starts_with('[')
            {
                return None;
            }

            Some(line.to_string())
        })
        .collect();

    Ok(screens)
}
