//! Audio output with rodio on its own thread (the output stream is not `Send`).
//! The order is decided by the pure `PlaybackQueue` from the core.
//!
//! The output stream (`MixerDeviceSink`) and every `Player` live in this thread
//! for as long as they play; dropping either one would silence the sound without
//! an error. Errors (file, codec, output device) are logged and, for previews,
//! returned to the UI.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ps2_tracker_core::audiofmt::{self, AudioKind};
use ps2_tracker_core::voice::{PlaybackQueue, QueueAction, QueueItem};
use rodio::cpal::traits::HostTrait;
use rodio::mixer::Mixer;
use rodio::{DeviceSinkBuilder, DeviceTrait, MixerDeviceSink, Player, Source};

type Reply = mpsc::Sender<Result<String, String>>;

enum Cmd {
    Play {
        file: PathBuf,
        volume: f32,
        priority: u8,
    },
    Preview {
        file: PathBuf,
        volume: f32,
        reply: Reply,
    },
    Configure {
        device: String,
        master: f32,
        max_concurrent: usize,
    },
    /// Stops everything playing or queued from files under this folder.
    StopUnder(PathBuf),
}

#[derive(Clone)]
pub struct AudioHandle {
    tx: mpsc::Sender<Cmd>,
}

impl AudioHandle {
    pub fn spawn(device: String, master: f32, max_concurrent: usize) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || run(rx, device, master, max_concurrent))
            .expect("audio thread starts");
        Self { tx }
    }

    pub fn play(&self, file: PathBuf, volume: f32, priority: u8) {
        if self
            .tx
            .send(Cmd::Play {
                file,
                volume,
                priority,
            })
            .is_err()
        {
            tracing::error!("audio thread is not running");
        }
    }

    /// Plays a file right away. Waits for the audio thread and returns what was
    /// played (codec, device) or why it could not be played.
    pub fn preview(&self, file: PathBuf, volume: f32) -> Result<String, String> {
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Cmd::Preview {
                file,
                volume,
                reply,
            })
            .map_err(|_| "audio thread is not running".to_string())?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "audio thread did not answer".to_string())?
    }

    pub fn stop_under(&self, dir: PathBuf) {
        let _ = self.tx.send(Cmd::StopUnder(dir));
    }

    pub fn configure(&self, device: String, master: f32, max_concurrent: usize) {
        let _ = self.tx.send(Cmd::Configure {
            device,
            master,
            max_concurrent,
        });
    }
}

fn device_name(d: &rodio::cpal::Device) -> String {
    d.description()
        .map(|x| x.name().to_owned())
        .unwrap_or_else(|_| "?".into())
}

/// Output device names (under PipeWire usually "pipewire", "pulse", "default" and the ALSA devices).
pub fn output_devices() -> Vec<String> {
    let host = rodio::cpal::default_host();
    let mut v: Vec<String> = host
        .output_devices()
        .map(|it| {
            it.filter_map(|d| d.description().ok().map(|x| x.name().to_owned()))
                .collect()
        })
        .unwrap_or_default();
    // the UI offers "System default (PipeWire)" instead of ALSA "default"
    v.retain(|n| !ps2_tracker_core::config::is_default_device_name(n));
    v.sort();
    v.dedup();
    v
}

/// Logs host, default device and all output devices (startup diagnostics).
pub fn log_devices() {
    let host = rodio::cpal::default_host();
    let default = host
        .default_output_device()
        .map(|d| device_name(&d))
        .unwrap_or_else(|| "(none)".into());
    tracing::info!(host = ?host.id(), default = %default, devices = ?output_devices(), "audio devices");
}

/// An open output. `beats` counts 100 ms periods the device has pulled from the
/// mixer (silent heartbeat source): if it stops, the stream is dead (e.g. its
/// callback thread panicked or ALSA stopped) and the output is reopened.
pub struct Output {
    /// Keeps the cpal stream alive; `None` for a virtual output (tests).
    _sink: Option<MixerDeviceSink>,
    mixer: Mixer,
    pub device: String,
    broken: Arc<AtomicBool>,
    beats: Arc<AtomicU64>,
}

impl Output {
    fn new(
        sink: Option<MixerDeviceSink>,
        mixer: Mixer,
        device: String,
        broken: Arc<AtomicBool>,
    ) -> Self {
        let beats = Arc::new(AtomicU64::new(0));
        let b = beats.clone();
        // keeps the mixer busy, so the device always pulls, and proves it does
        mixer.add(
            rodio::source::Zero::new(
                NonZero::new(1).expect("1"),
                NonZero::new(48_000).expect("48000"),
            )
            .periodic_access(Duration::from_millis(100), move |_| {
                b.fetch_add(1, Ordering::Relaxed);
            }),
        );
        Self {
            _sink: sink,
            mixer,
            device,
            broken,
            beats,
        }
    }

    /// Output without a device: the caller pulls the samples (tests).
    #[cfg(test)]
    pub fn virtual_output(channels: u16, rate: u32) -> (Self, rodio::mixer::MixerSource) {
        let (mixer, source) = rodio::mixer::mixer(
            NonZero::new(channels).expect("channels > 0"),
            NonZero::new(rate).expect("rate > 0"),
        );
        let o = Self::new(
            None,
            mixer,
            "virtual".into(),
            Arc::new(AtomicBool::new(false)),
        );
        (o, source)
    }

    pub fn mixer(&self) -> &Mixer {
        &self.mixer
    }
}

fn open_device(dev: rodio::cpal::Device) -> Result<Output, String> {
    let name = device_name(&dev);
    let broken = Arc::new(AtomicBool::new(false));
    let flag = broken.clone();
    let err_name = name.clone();
    let mut sink = DeviceSinkBuilder::from_device(dev)
        .map_err(|e| format!("{name}: {e}"))?
        .with_error_callback(move |e| {
            tracing::error!(device = %err_name, error = %e, "audio stream error");
            flag.store(true, Ordering::Relaxed);
        })
        .open_sink_or_fallback()
        .map_err(|e| format!("{name}: {e}"))?;
    sink.log_on_drop(false);
    let c = sink.config();
    tracing::info!(device = %name, rate = c.sample_rate().get(), channels = c.channel_count().get(), format = ?c.sample_format(), buffer = ?c.buffer_size(), "audio output opened");
    let mixer = sink.mixer().clone();
    Ok(Output::new(Some(sink), mixer, name, broken))
}

/// PipeWire's ALSA PCM ("PipeWire Sound Server" = `pipewire`, else the
/// "Default ALSA Output (currently PipeWire …)" alias), if pipewire-alsa is installed.
fn pipewire_device(host: &rodio::cpal::Host) -> Option<rodio::cpal::Device> {
    let devs: Vec<rodio::cpal::Device> = host.output_devices().ok()?.collect();
    let name = |d: &rodio::cpal::Device| device_name(d).to_lowercase();
    let pick = |want: &dyn Fn(&str) -> bool| devs.iter().find(|d| want(&name(d))).cloned();
    pick(&|n| n.starts_with("pipewire")).or_else(|| pick(&|n| n.contains("pipewire")))
}

/// Opens the configured device; if it is gone or fails, the system default
/// (on Linux PipeWire's ALSA device first), then any other real output device.
pub fn open_output(device: &str) -> Result<Output, String> {
    let host = rodio::cpal::default_host();
    let mut errors = Vec::new();
    // "Default Audio Device" (cpal's ALSA "default") means the system default:
    // PipeWire first, so the dmix attempt ("unable to open slave") is skipped
    let device = if ps2_tracker_core::config::is_default_device_name(device) {
        ""
    } else {
        device
    };
    if !device.is_empty() {
        let dev = host
            .output_devices()
            .ok()
            .and_then(|mut it| it.find(|d| d.description().is_ok_and(|x| x.name() == device)));
        match dev {
            Some(d) => match open_device(d) {
                Ok(o) => return Ok(o),
                Err(e) => {
                    tracing::warn!(error = %e, "selected output device failed, using default");
                    errors.push(e);
                }
            },
            None => {
                tracing::warn!(device, available = ?output_devices(), "selected output device not found, using default");
                errors.push(format!("device \"{device}\" not found"));
            }
        }
    }
    // Linux: "default" can resolve to ALSA dmix on the hardware even with
    // PipeWire running ("unable to open slave"); PipeWire's ALSA device is the
    // reliable system default then.
    if device.is_empty()
        && cfg!(target_os = "linux")
        && let Some(d) = pipewire_device(&host)
    {
        match open_device(d) {
            Ok(o) => return Ok(o),
            Err(e) => {
                tracing::warn!(error = %e, "PipeWire ALSA device failed, trying the ALSA default");
                errors.push(e);
            }
        }
    }
    match host.default_output_device() {
        Some(d) => match open_device(d) {
            Ok(o) => return Ok(o),
            Err(e) => errors.push(format!("default device {e}")),
        },
        None => errors.push("no default output device".into()),
    }
    if let Ok(devs) = host.output_devices() {
        for d in devs {
            let real = d
                .description()
                .is_ok_and(|x| x.driver().is_some_and(|drv| drv != "null"));
            if real && let Ok(o) = open_device(d) {
                return Ok(o);
            }
        }
    }
    let msg = format!(
        "no audio output: {} (is pipewire-alsa or pulseaudio-alsa installed?)",
        errors.join("; ")
    );
    tracing::error!("{msg}");
    Err(msg)
}

/// Wraps a source so a panic while decoding ends this sound instead of killing
/// the device's callback thread (which would silence every later sound).
pub struct Guarded<S> {
    inner: Option<S>,
    name: String,
}

impl<S: Source> Guarded<S> {
    pub fn new(inner: S, name: String) -> Self {
        Self {
            inner: Some(inner),
            name,
        }
    }
}

impl<S: Source> Iterator for Guarded<S> {
    type Item = rodio::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        let inner = self.inner.as_mut()?;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.next())) {
            Ok(v) => v,
            Err(_) => {
                tracing::error!(file = %self.name, "decoder panicked, sound stopped");
                self.inner = None;
                None
            }
        }
    }
}

impl<S: Source> Source for Guarded<S> {
    fn current_span_len(&self) -> Option<usize> {
        // after a panic: an empty span ends the source
        self.inner
            .as_ref()
            .map_or(Some(0), |s| s.current_span_len())
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.inner
            .as_ref()
            .map_or(NonZero::new(1).expect("1"), |s| s.channels())
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner
            .as_ref()
            .map_or(NonZero::new(48_000).expect("48000"), |s| s.sample_rate())
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.as_ref().and_then(|s| s.total_duration())
    }
}

pub type FileDecoder = rodio::Decoder<BufReader<File>>;

/// Opens and decodes a file. The error names the file, and the codec when it is
/// not supported.
pub fn open_audio(file: &Path) -> Result<(FileDecoder, AudioKind), String> {
    let shown = file.display();
    let meta = std::fs::metadata(file).map_err(|e| format!("cannot open {shown}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("not a file: {shown}"));
    }
    let kind = audiofmt::sniff_file(file).map_err(|e| format!("cannot read {shown}: {e}"))?;
    if kind == AudioKind::Empty {
        return Err(format!("file is empty: {shown}"));
    }
    let f = File::open(file).map_err(|e| format!("cannot open {shown}: {e}"))?;
    rodio::Decoder::try_from(f).map(|d| (d, kind)).map_err(|e| {
        if kind.supported() {
            format!("cannot decode {shown} ({}): {e}", kind.label())
        } else {
            format!(
                "{shown}: {} is not supported (supported: Ogg Vorbis, MP3, FLAC, WAV). \
                     Convert it, e.g. ffmpeg -i in.ogg -c:a libvorbis out.ogg ({e})",
                kind.label()
            )
        }
    })
}

/// Decodes the start of a file completely (finds broken streams that open fine).
pub fn check_audio(file: &Path) -> Result<String, String> {
    let (d, kind) = open_audio(file)?;
    let rate = d.sample_rate().get();
    let channels = d.channels().get();
    let dur = d.total_duration();
    let n = d.take(rate as usize * channels as usize).count();
    if n == 0 {
        return Err(format!(
            "{} ({}) decodes to no audio",
            file.display(),
            kind.label()
        ));
    }
    Ok(format!(
        "{}, {rate} Hz, {channels} ch{}",
        kind.label(),
        dur.map(|d| format!(", {:.1} s", d.as_secs_f32()))
            .unwrap_or_default()
    ))
}

type Opener = Box<dyn FnMut(&str) -> Result<Output, String> + Send>;

/// Holds the output and reopens it when it is missing, reported broken by the
/// stream's error callback, or stalled (heartbeat stopped). Covers an app that
/// started before PipeWire, a PipeWire restart and a dead callback thread.
/// Retries at most every 5 s, except when `force` (previews).
pub struct OutputSlot {
    device: String,
    out: Option<Output>,
    last_try: Option<u64>,
    opener: Opener,
    /// (heartbeat value, time it last changed)
    beat: (u64, u64),
}

/// The heartbeat must advance within this time (it ticks every 100 ms).
const STALL_MS: u64 = 2000;

impl OutputSlot {
    pub fn new(device: String, opener: Opener) -> Self {
        Self {
            device,
            out: None,
            last_try: None,
            opener,
            beat: (0, 0),
        }
    }

    fn check_alive(&mut self, now: u64) -> bool {
        let Some(o) = &self.out else { return true };
        if o.broken.load(Ordering::Relaxed) {
            tracing::warn!(device = %o.device, "audio stream reported an error, reopening");
            self.out = None;
            self.last_try = None; // reopen right away
            return false;
        }
        let beats = o.beats.load(Ordering::Relaxed);
        if beats != self.beat.0 {
            self.beat = (beats, now);
        } else if now.saturating_sub(self.beat.1) > STALL_MS {
            tracing::error!(device = %o.device, beats, "audio output stalled (device stopped pulling samples), reopening");
            self.out = None;
            self.last_try = None; // reopen right away
            return false;
        }
        true
    }

    pub fn get(&mut self, force: bool, now: u64) -> Result<&Output, String> {
        self.check_alive(now);
        if self.out.is_none() {
            let due = self.last_try.is_none_or(|t| now.saturating_sub(t) >= 5000);
            if !force && !due {
                return Err("no audio output (retrying)".into());
            }
            self.last_try = Some(now);
            let o = (self.opener)(&self.device)?;
            self.beat = (o.beats.load(Ordering::Relaxed), now);
            self.out = Some(o);
        }
        self.out.as_ref().ok_or_else(|| "no audio output".into())
    }

    fn reset(&mut self, device: String, now: u64) {
        self.out = None; // close the old stream first
        self.device = device;
        self.last_try = None;
        let _ = self.get(true, now);
    }
}

/// Audio state machine, driven by the audio thread (and directly by tests).
/// Time comes in from outside (`now` in ms).
pub struct Engine {
    output: OutputSlot,
    queue: PlaybackQueue,
    pending: HashMap<u64, (PathBuf, f32)>,
    players: HashMap<u64, (Player, PathBuf)>,
    preview: Option<Player>,
    next_id: u64,
    master: f32,
}

impl Engine {
    pub fn new(output: OutputSlot, master: f32, max_concurrent: usize) -> Self {
        Self {
            output,
            queue: PlaybackQueue::new(max_concurrent),
            pending: HashMap::new(),
            players: HashMap::new(),
            preview: None,
            next_id: 1,
            master,
        }
    }

    pub fn open(&mut self, now: u64) {
        let _ = self.output.get(true, now);
    }

    /// Queues a sound; returns its queue id.
    pub fn play(&mut self, file: PathBuf, volume: f32, priority: u8, now: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let duration_ms = match open_audio(&file) {
            Ok((d, _)) => d.total_duration().map(|d| d.as_millis() as u64),
            Err(e) => {
                tracing::warn!("{e}");
                None
            }
        }
        .unwrap_or(2500)
        .clamp(100, 15_000);
        tracing::debug!(id, file = %file.display(), volume, priority, duration_ms, "sound queued");
        self.pending.insert(id, (file, volume));
        let actions = self.queue.push(
            QueueItem {
                id,
                priority,
                duration_ms,
            },
            now,
        );
        self.apply(actions, now);
        id
    }

    /// Plays at once, outside the queue (and without cooldowns).
    pub fn preview(&mut self, file: &Path, volume: f32, now: u64) -> Result<String, String> {
        let res = (|| {
            let info = check_audio(file)?;
            let (src, _) = open_audio(file)?;
            let master = self.master;
            let out = self.output.get(true, now)?;
            if let Some(p) = self.preview.take() {
                p.stop();
            }
            let p = Player::connect_new(out.mixer());
            p.set_volume((volume * master).clamp(0.0, 2.0));
            p.append(Guarded::new(src, file.display().to_string()));
            let device = out.device.clone();
            self.preview = Some(p);
            Ok(format!("{info} → {device}"))
        })();
        match &res {
            Ok(info) => {
                tracing::info!(file = %file.display(), volume, master = self.master, "preview: {info}")
            }
            Err(e) => tracing::warn!("preview failed: {e}"),
        }
        res
    }

    pub fn configure(&mut self, device: String, master: f32, max_concurrent: usize, now: u64) {
        tracing::info!(device, master, max_concurrent, "audio reconfigured");
        self.master = master;
        for (_, (p, _)) in self.players.drain() {
            p.stop();
        }
        if let Some(p) = self.preview.take() {
            p.stop();
        }
        self.pending.clear();
        self.queue = PlaybackQueue::new(max_concurrent);
        self.output.reset(device, now);
    }

    /// Stops sounds from a folder that is going away (removed pack); the
    /// preview is stopped too (its path is not kept).
    pub fn stop_under(&mut self, dir: &Path) {
        let mut n = 0;
        for (p, file) in self.players.values() {
            if file.starts_with(dir) {
                p.stop();
                n += 1;
            }
        }
        let before = self.pending.len();
        self.pending.retain(|_, (f, _)| !f.starts_with(dir));
        n += before - self.pending.len();
        if let Some(p) = self.preview.take() {
            p.stop();
        }
        tracing::info!(dir = %dir.display(), stopped = n, "sounds of a removed pack stopped");
    }

    pub fn tick(&mut self, now: u64) {
        let mut actions = Vec::new();
        if !self.output.check_alive(now) {
            // the stream is gone: its players will never finish on their own
            for (id, (p, file)) in self.players.drain() {
                p.stop();
                tracing::warn!(id, file = %file.display(), "sound lost with the audio stream");
                actions.extend(self.queue.finished(id, now));
            }
            self.preview = None;
        }
        let done: Vec<u64> = self
            .players
            .iter()
            .filter(|(_, (p, _))| p.empty())
            .map(|(id, _)| *id)
            .collect();
        for id in done {
            if let Some((_, file)) = self.players.remove(&id) {
                tracing::debug!(id, file = %file.display(), "sound finished");
            }
            actions.extend(self.queue.finished(id, now));
        }
        actions.extend(self.queue.tick(now));
        self.apply(actions, now);
    }

    fn apply(&mut self, actions: Vec<QueueAction>, now: u64) {
        let mut actions: std::collections::VecDeque<QueueAction> = actions.into();
        while let Some(a) = actions.pop_front() {
            match a {
                QueueAction::Play(id) => {
                    let Some((file, volume)) = self.pending.remove(&id) else {
                        continue;
                    };
                    let master = self.master;
                    let output = &mut self.output;
                    let started = open_audio(&file).and_then(|(src, kind)| {
                        let out = output.get(false, now)?;
                        let pl = Player::connect_new(out.mixer());
                        pl.set_volume((volume * master).clamp(0.0, 2.0));
                        pl.append(Guarded::new(src, file.display().to_string()));
                        tracing::debug!(id, file = %file.display(), codec = kind.label(), device = %out.device, "sound started");
                        Ok(pl)
                    });
                    match started {
                        Ok(pl) => {
                            self.players.insert(id, (pl, file));
                        }
                        Err(e) => {
                            tracing::warn!(id, "sound not played: {e}");
                            // not playable: free the slot immediately
                            actions.extend(self.queue.finished(id, now));
                        }
                    }
                }
                QueueAction::Stop(id) => {
                    if let Some((p, file)) = self.players.remove(&id) {
                        tracing::debug!(id, file = %file.display(), "sound stopped for a higher priority one");
                        p.stop();
                    }
                }
                QueueAction::Drop(id) => {
                    if let Some((file, _)) = self.pending.remove(&id) {
                        tracing::debug!(id, file = %file.display(), "sound dropped (queue full or waited too long)");
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn playing(&self) -> usize {
        self.players.len()
    }
}

struct Settings {
    device: String,
    master: f32,
    max_concurrent: usize,
}

fn run(rx: mpsc::Receiver<Cmd>, device: String, master: f32, max_concurrent: usize) {
    let start = Instant::now();
    let ms = move || start.elapsed().as_millis() as u64;
    let mut settings = Settings {
        device,
        master,
        max_concurrent,
    };
    // a panic must not leave the app without sound: log it and start over
    loop {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_engine(&rx, &mut settings, &ms)
        }));
        match res {
            Ok(()) => return, // channel closed: app is exiting
            Err(_) => {
                tracing::error!("audio thread panicked, restarting audio");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn run_engine(rx: &mpsc::Receiver<Cmd>, settings: &mut Settings, ms: &dyn Fn() -> u64) {
    let mut engine = Engine::new(
        OutputSlot::new(settings.device.clone(), Box::new(open_output)),
        settings.master,
        settings.max_concurrent,
    );
    engine.open(ms());
    loop {
        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(Cmd::Play {
                file,
                volume,
                priority,
            }) => {
                engine.play(file, volume, priority, ms());
            }
            Ok(Cmd::Preview {
                file,
                volume,
                reply,
            }) => {
                let _ = reply.send(engine.preview(&file, volume, ms()));
            }
            Ok(Cmd::Configure {
                device,
                master,
                max_concurrent,
            }) => {
                settings.device = device.clone();
                settings.master = master;
                settings.max_concurrent = max_concurrent;
                engine.configure(device, master, max_concurrent, ms());
            }
            Ok(Cmd::StopUnder(dir)) => engine.stop_under(&dir),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        engine.tick(ms());
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, Guarded, Output, OutputSlot, check_audio, open_audio};
    use rodio::Source;
    use rodio::mixer::MixerSource;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/ps2-tracker-core/tests/fixtures")
    }

    /// Ogg/Vorbis, MP3, FLAC and WAV must decode (self-generated tones).
    #[test]
    fn decodes_all_supported_formats() {
        let dir = fixtures().join("audio");
        for ext in ["ogg", "mp3", "flac", "wav"] {
            let (d, _) = open_audio(&dir.join(format!("tone.{ext}")))
                .unwrap_or_else(|e| panic!("{ext} not decodable: {e}"));
            let rate = d.sample_rate();
            let n = d.take(100_000).filter(|s| s.abs() > 0.01).count();
            assert!(n > 1000, "{ext}: too few audible samples ({n}, {rate:?})");
        }
    }

    #[test]
    fn decodes_recursion_fixture_with_special_chars() {
        let f = fixtures().join("recursion/Testpack/Don't Stop!.ogg");
        let info = check_audio(&f).unwrap();
        assert!(info.starts_with("Ogg Vorbis"), "{info}");
    }

    #[test]
    fn errors_name_the_problem() {
        let tmp = std::env::temp_dir().join(format!("ps2t-audio-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let missing = open_audio(&tmp.join("nope.ogg")).err().unwrap();
        assert!(
            missing.contains("cannot open") && missing.contains("nope.ogg"),
            "{missing}"
        );
        let empty = tmp.join("empty.ogg");
        std::fs::write(&empty, b"").unwrap();
        assert!(open_audio(&empty).err().unwrap().contains("empty"));
        // Ogg Opus header: the message must name the codec
        let mut opus = b"OggS\x00\x02".to_vec();
        opus.resize(26, 0);
        opus.extend_from_slice(&[1, 19]);
        opus.extend_from_slice(b"OpusHead\x01\x01\x38\x01\x80\xbb\x00\x00\x00\x00\x00");
        let f = tmp.join("opus.ogg");
        std::fs::write(&f, &opus).unwrap();
        let err = open_audio(&f).err().unwrap();
        assert!(err.contains("Ogg Opus is not supported"), "{err}");
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// Engine on a virtual output; the test plays the device clock.
    struct Rig {
        engine: Engine,
        source: Arc<Mutex<Option<MixerSource>>>,
        opens: Arc<AtomicUsize>,
        now: u64,
    }

    const CH: u16 = 2;
    const RATE: u32 = 48_000;

    impl Rig {
        fn new(max_concurrent: usize) -> Self {
            let source = Arc::new(Mutex::new(None));
            let opens = Arc::new(AtomicUsize::new(0));
            let (s, o) = (source.clone(), opens.clone());
            let slot = OutputSlot::new(
                String::new(),
                Box::new(move |_| {
                    o.fetch_add(1, Ordering::SeqCst);
                    let (out, src) = Output::virtual_output(CH, RATE);
                    *s.lock().unwrap() = Some(src);
                    Ok(out)
                }),
            );
            let mut engine = Engine::new(slot, 1.0, max_concurrent);
            engine.open(0);
            Self {
                engine,
                source,
                opens,
                now: 0,
            }
        }

        /// Lets `ms` pass in 40 ms steps (like the audio thread); returns the
        /// number of audible samples the "device" received. `pull = false`
        /// simulates a dead device.
        fn run(&mut self, ms: u64, pull: bool) -> usize {
            let mut audible = 0;
            for _ in 0..ms / 40 {
                if pull {
                    let mut g = self.source.lock().unwrap();
                    let src = g.as_mut().unwrap();
                    for _ in 0..(RATE as usize * CH as usize / 25) {
                        if src.next().is_some_and(|s| s.abs() > 0.01) {
                            audible += 1;
                        }
                    }
                }
                self.now += 40;
                self.engine.tick(self.now);
            }
            audible
        }
    }

    fn tone() -> PathBuf {
        fixtures().join("audio/tone.ogg")
    }

    /// Reported bug: one sound, then silence. Ten sounds in a row must all
    /// play, each after the previous one has ended (max. 1 at once).
    #[test]
    fn ten_sounds_in_a_row_all_play() {
        let mut rig = Rig::new(1);
        for i in 0..10 {
            rig.engine.play(tone(), 1.0, 50, rig.now);
            assert_eq!(rig.engine.playing(), 1, "sound {i} did not start");
            let audible = rig.run(600, true);
            assert!(audible > 4000, "sound {i}: only {audible} audible samples");
            assert_eq!(rig.engine.playing(), 0, "sound {i} did not finish");
        }
        assert_eq!(
            rig.opens.load(Ordering::SeqCst),
            1,
            "output reopened without need"
        );
    }

    /// Five at once with max. 1 at once: the running one plays, two wait and
    /// follow, the rest is dropped; afterwards new sounds play again.
    #[test]
    fn simultaneous_sounds_queue_and_drain() {
        let mut rig = Rig::new(1);
        let ids: Vec<u64> = (0..5)
            .map(|_| rig.engine.play(tone(), 1.0, 50, rig.now))
            .collect();
        assert_eq!(ids.len(), 5);
        assert_eq!(rig.engine.playing(), 1);
        // 3 sounds of ~200 ms each in sequence
        let audible = rig.run(1400, true);
        assert!(audible > 3 * 4000, "{audible}");
        assert_eq!(rig.engine.playing(), 0);
        rig.engine.play(tone(), 1.0, 50, rig.now);
        assert_eq!(rig.engine.playing(), 1, "queue stuck after a burst");
        assert!(rig.run(600, true) > 4000);
    }

    #[test]
    fn preview_works_after_playback() {
        let mut rig = Rig::new(1);
        rig.engine.play(tone(), 1.0, 50, rig.now);
        assert!(rig.run(600, true) > 4000);
        let info = rig.engine.preview(&tone(), 1.0, rig.now).unwrap();
        assert!(
            info.contains("Ogg Vorbis") && info.ends_with("virtual"),
            "{info}"
        );
        assert!(rig.run(600, true) > 4000, "preview silent");
        // and a queued sound after the preview
        rig.engine.play(tone(), 1.0, 50, rig.now);
        assert!(rig.run(600, true) > 4000);
    }

    /// The device stops pulling (dead callback thread): the output is reopened
    /// and the next sound plays on the new one.
    #[test]
    fn stalled_output_is_reopened() {
        let mut rig = Rig::new(1);
        rig.engine.play(tone(), 1.0, 50, rig.now);
        rig.run(2400, false);
        assert_eq!(rig.engine.playing(), 0, "lost sound still blocks the queue");
        rig.engine.play(tone(), 1.0, 50, rig.now);
        assert_eq!(rig.opens.load(Ordering::SeqCst), 2);
        assert!(rig.run(600, true) > 4000);
    }

    /// Real output device (ALSA default, e.g. PipeWire through pipewire-alsa):
    /// ten sounds and a preview, in real time. Run by hand:
    /// `cargo test -p ps2-tracker real_output -- --ignored --nocapture`
    #[test]
    #[ignore = "needs an audio output device"]
    fn real_output_ten_sounds_and_preview() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .try_init();
        let opens = Arc::new(AtomicUsize::new(0));
        let o = opens.clone();
        let slot = OutputSlot::new(
            String::new(),
            Box::new(move |d| {
                o.fetch_add(1, Ordering::SeqCst);
                super::open_output(d)
            }),
        );
        let start = std::time::Instant::now();
        let now = || start.elapsed().as_millis() as u64;
        let mut engine = Engine::new(slot, 1.0, 1);
        engine.open(now());
        let wait = |engine: &mut Engine, ms: u64| {
            let until = now() + ms;
            while now() < until {
                std::thread::sleep(std::time::Duration::from_millis(40));
                engine.tick(now());
            }
        };
        for i in 0..10 {
            engine.play(tone(), 0.2, 50, now());
            assert_eq!(engine.playing(), 1, "sound {i} did not start");
            wait(&mut engine, 700);
            assert_eq!(
                engine.playing(),
                0,
                "sound {i} never finished (device not pulling)"
            );
        }
        engine.preview(&tone(), 0.2, now()).unwrap();
        wait(&mut engine, 700);
        engine.play(tone(), 0.2, 50, now());
        wait(&mut engine, 700);
        assert_eq!(engine.playing(), 0);
        assert_eq!(opens.load(Ordering::SeqCst), 1, "output had to be reopened");
    }

    /// End to end with the real pack layout (synthetic tone): import, then the
    /// paths the preview commands resolve exist and decode, and play.
    #[test]
    fn imported_recursion_pack_previews() {
        use ps2_tracker_core::packloader;
        let base = std::env::temp_dir().join(format!("ps2t-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src");
        std::fs::create_dir_all(src.join("ArnoldVoicePack")).unwrap();
        let files = [
            "Rubber baby buggy bumpers!.ogg",
            "I'llBeBack.ogg",
            "Stop Whining.ogg",
        ];
        let keys = ["HEAD_SHOT", "DOUBLE_KILL", "REVENGE"];
        let mut xml = String::from(
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<AchievementSettings><ACHIEVEMENTDATA><AUTHOR>x</AUTHOR><DESCRIPTION>y</DESCRIPTION><DATA>\r\n",
        );
        for (k, f) in keys.iter().zip(files) {
            std::fs::copy(tone(), src.join("ArnoldVoicePack").join(f)).unwrap();
            xml.push_str(&format!(
                "<ACHIEVEMENT><NAME>{k}</NAME><PATH>.\\ArnoldVoicePack\\{}</PATH><IMAGE_INGAME>true</IMAGE_INGAME><IMAGE_STREAMING>true</IMAGE_STREAMING><SOUND>true</SOUND></ACHIEVEMENT>\r\n",
                f.replace('\'', "&apos;")
            ));
        }
        xml.push_str("</DATA></ACHIEVEMENTDATA></AchievementSettings>\r\n");
        std::fs::write(src.join("ArnoldVoicePk.rtst_vpk"), xml).unwrap();
        let (_, pack) =
            packloader::import(&src.join("ArnoldVoicePk.rtst_vpk"), &base.join("lib")).unwrap();
        let mut rig = Rig::new(1);
        for k in keys {
            let e = pack.manifest.events.iter().find(|e| e.event == k).unwrap();
            let path = pack.file_path(&e.files[0]).unwrap();
            assert!(path.starts_with(base.join("lib")));
            let info = rig.engine.preview(&path, 1.0, rig.now).unwrap();
            assert!(info.starts_with("Ogg Vorbis"), "{info}");
            assert!(rig.run(600, true) > 4000, "{k} silent");
        }
        let _ = std::fs::remove_dir_all(base);
    }

    /// A decoder panic ends that sound only; the mixer keeps going.
    #[test]
    fn decoder_panic_is_contained() {
        struct Boom(u32);
        impl Iterator for Boom {
            type Item = rodio::Sample;
            fn next(&mut self) -> Option<Self::Item> {
                self.0 += 1;
                assert!(self.0 < 100, "simulated decoder bug");
                Some(0.5)
            }
        }
        impl Source for Boom {
            fn current_span_len(&self) -> Option<usize> {
                None
            }
            fn channels(&self) -> rodio::ChannelCount {
                std::num::NonZero::new(1).unwrap()
            }
            fn sample_rate(&self) -> rodio::SampleRate {
                std::num::NonZero::new(48_000).unwrap()
            }
            fn total_duration(&self) -> Option<std::time::Duration> {
                None
            }
        }
        let mut rig = Rig::new(1);
        let mixer = rig.engine.output.get(true, 0).unwrap().mixer().clone();
        mixer.add(Guarded::new(Boom(0), "boom".into()));
        rig.run(200, true);
        rig.engine.play(tone(), 1.0, 50, rig.now);
        assert!(rig.run(600, true) > 4000);
    }
}
