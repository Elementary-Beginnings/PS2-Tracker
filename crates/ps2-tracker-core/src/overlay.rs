//! Overlay placement: anchor + offset on a monitor, always fully inside it,
//! and the size step (S/M/L/XL) from the monitor height. Pure, testable.

/// Corner/edge of the monitor the overlay sticks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    Center,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    pub const ALL: [&'static str; 7] = [
        "top-left",
        "top-center",
        "top-right",
        "center",
        "bottom-left",
        "bottom-center",
        "bottom-right",
    ];

    /// Unknown names fall back to top-right (the default).
    pub fn parse(s: &str) -> Self {
        match s {
            "top-left" => Self::TopLeft,
            "top-center" => Self::TopCenter,
            "center" => Self::Center,
            "bottom-left" => Self::BottomLeft,
            "bottom-center" => Self::BottomCenter,
            "bottom-right" => Self::BottomRight,
            _ => Self::TopRight,
        }
    }
}

/// Top-left corner of the overlay window. `area` = monitor (x, y, width,
/// height) in physical pixels, `size` = window size, `offset` = distance from
/// the anchored edges, positive towards the middle of the monitor (for the
/// centred axes it shifts right/down). The result always lies fully inside the
/// area; a window larger than the area sticks to its top-left corner.
pub fn place(
    area: (i32, i32, u32, u32),
    size: (u32, u32),
    anchor: Anchor,
    offset: (i32, i32),
) -> (i32, i32) {
    let (ax, ay, aw, ah) = (
        i64::from(area.0),
        i64::from(area.1),
        i64::from(area.2),
        i64::from(area.3),
    );
    let (w, h) = (i64::from(size.0), i64::from(size.1));
    let (ox, oy) = (i64::from(offset.0), i64::from(offset.1));
    use Anchor::*;
    let x = match anchor {
        TopLeft | BottomLeft => ax + ox,
        TopRight | BottomRight => ax + aw - w - ox,
        TopCenter | Center | BottomCenter => ax + (aw - w).div_euclid(2) + ox,
    };
    let y = match anchor {
        TopLeft | TopCenter | TopRight => ay + oy,
        BottomLeft | BottomCenter | BottomRight => ay + ah - h - oy,
        Center => ay + (ah - h).div_euclid(2) + oy,
    };
    let clamp = |v: i64, lo: i64, len: i64, win: i64| v.min(lo + len - win).max(lo);
    (clamp(x, ax, aw, w) as i32, clamp(y, ay, ah, h) as i32)
}

/// Offset (as stored) that puts the window at `pos`; inverse of [`place`]
/// after clamping `pos` into the area. Used when the window is dragged.
pub fn offset_for(
    area: (i32, i32, u32, u32),
    size: (u32, u32),
    anchor: Anchor,
    pos: (i32, i32),
) -> (i32, i32) {
    let (ax, ay, aw, ah) = (
        i64::from(area.0),
        i64::from(area.1),
        i64::from(area.2),
        i64::from(area.3),
    );
    let (w, h) = (i64::from(size.0), i64::from(size.1));
    let clamp = |v: i64, lo: i64, len: i64, win: i64| v.min(lo + len - win).max(lo);
    let x = clamp(i64::from(pos.0), ax, aw, w);
    let y = clamp(i64::from(pos.1), ay, ah, h);
    use Anchor::*;
    let ox = match anchor {
        TopLeft | BottomLeft => x - ax,
        TopRight | BottomRight => ax + aw - w - x,
        TopCenter | Center | BottomCenter => x - (ax + (aw - w).div_euclid(2)),
    };
    let oy = match anchor {
        TopLeft | TopCenter | TopRight => y - ay,
        BottomLeft | BottomCenter | BottomRight => ay + ah - h - y,
        Center => y - (ay + (ah - h).div_euclid(2)),
    };
    (ox as i32, oy as i32)
}

/// One object (stats overlay or pop-up) while the move layer is open: where it
/// is (absolute physical pixels) and on which monitor. Nothing is written to
/// the settings or the real windows until the session is saved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MovedObject {
    /// Monitor name ("" = primary, as in the settings).
    pub monitor: String,
    /// Monitor area (x, y, width, height), physical pixels.
    pub area: (i32, i32, u32, u32),
    pub anchor: String,
    pub size: (u32, u32),
    /// Top-left corner, absolute physical pixels, always inside `area`.
    pub pos: (i32, i32),
}

impl MovedObject {
    /// From the stored anchor/offset.
    pub fn new(
        monitor: &str,
        area: (i32, i32, u32, u32),
        anchor: &str,
        offset: (i32, i32),
        size: (u32, u32),
    ) -> Self {
        Self {
            monitor: monitor.to_owned(),
            area,
            anchor: anchor.to_owned(),
            size,
            pos: place(area, size, Anchor::parse(anchor), offset),
        }
    }

    /// Offset to store for the current position (relative to the monitor and
    /// the anchor, as before).
    pub fn offset(&self) -> (i32, i32) {
        offset_for(self.area, self.size, Anchor::parse(&self.anchor), self.pos)
    }

    /// Sets the position, kept fully on the monitor.
    pub fn set_pos(&mut self, pos: (i32, i32)) {
        let (ax, ay, aw, ah) = self.area;
        let clamp = |v: i32, lo: i32, len: u32, win: u32| {
            let hi = i64::from(lo) + i64::from(len) - i64::from(win);
            i64::from(v).min(hi).max(i64::from(lo)) as i32
        };
        self.pos = (
            clamp(pos.0, ax, aw, self.size.0),
            clamp(pos.1, ay, ah, self.size.1),
        );
    }

    pub fn nudge(&mut self, dx: i32, dy: i32) {
        self.set_pos((self.pos.0.saturating_add(dx), self.pos.1.saturating_add(dy)));
    }

    /// Moves the object to another monitor: same anchor and offset there.
    pub fn to_monitor(&mut self, monitor: &str, area: (i32, i32, u32, u32)) {
        let off = self.offset();
        self.monitor = monitor.to_owned();
        self.area = area;
        self.pos = place(area, self.size, Anchor::parse(&self.anchor), off);
    }

    /// Back to the default anchor and offset (on the current monitor).
    pub fn reset(&mut self, anchor: &str, offset: (i32, i32)) {
        self.anchor = anchor.to_owned();
        self.pos = place(self.area, self.size, Anchor::parse(anchor), offset);
    }
}

/// Size steps; "auto" picks by monitor height so the overlay stays readable
/// on 1440p and 4K (1080p → S, 1440p → M, 2160p → L, larger → XL).
pub fn size_factor(size: &str, monitor_height: u32) -> f32 {
    match size {
        "S" => 0.85,
        "M" => 1.0,
        "L" => 1.3,
        "XL" => 1.6,
        _ => match monitor_height {
            0..=1100 => 0.85,
            1101..=1700 => 1.0,
            1701..=2300 => 1.3,
            _ => 1.6,
        },
    }
}

/// Shortest display time of a pop-up (the UI uses the same floor).
pub const POPUP_MIN_MS: u64 = 800;
/// Fade-out of a pop-up in the UI (`src/popup/Popup.svelte`).
pub const POPUP_FADE_OUT_MS: u64 = 300;
/// Extra time before the window is hidden, so the fade-out is never cut off
/// by a webview whose timers run a little late.
pub const POPUP_HIDE_SLACK_MS: u64 = 250;

/// Whether the native pop-up window should be mapped. It is shown only while
/// a pop-up is on screen and hidden after its fade-out: an empty transparent
/// window that stays mapped can keep its last painted frame on some
/// compositors (WebKitGTK under KWin/XWayland), so the pack image stayed on
/// screen. The focus watcher and every other re-apply ask this gate.
#[derive(Debug, Default, Clone)]
pub struct PopupGate {
    /// Unix ms until the window may stay mapped (0 = no pop-up yet).
    until_ms: u64,
    /// Counts pop-ups; a hide scheduled for an older one is ignored.
    generation: u64,
}

impl PopupGate {
    /// A pop-up starts at `now_ms` and shows for `duration_ms` (at least
    /// [`POPUP_MIN_MS`]). Returns its generation and the time at which the
    /// window may be hidden again. A pop-up that replaces a running one
    /// extends the time.
    pub fn start(&mut self, now_ms: u64, duration_ms: u64) -> (u64, u64) {
        self.generation += 1;
        let until =
            now_ms + duration_ms.max(POPUP_MIN_MS) + POPUP_FADE_OUT_MS + POPUP_HIDE_SLACK_MS;
        self.until_ms = self.until_ms.max(until);
        (self.generation, self.until_ms)
    }

    /// Is a pop-up on screen (or fading out) at `now_ms`?
    pub fn active(&self, now_ms: u64) -> bool {
        now_ms < self.until_ms
    }

    /// The hide scheduled for pop-up `generation` is due at `now_ms`: true if
    /// the window should be hidden now (no newer pop-up, time is up).
    pub fn expire(&self, generation: u64, now_ms: u64) -> bool {
        generation == self.generation && !self.active(now_ms)
    }
}

/// The pop-up window is mapped while a pop-up shows, or in the move mode
/// (the move layer positions the objects).
pub fn popup_window_wanted(active: bool, moving: bool) -> bool {
    active || moving
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real two-monitor layout: DP-1 5120x1440 at y=1200 below DP-2.
    const DP1: (i32, i32, u32, u32) = (0, 1200, 5120, 1440);

    #[test]
    fn anchors_on_offset_monitor() {
        let s = (400, 300);
        assert_eq!(
            place(DP1, s, Anchor::TopRight, (24, 24)),
            (5120 - 400 - 24, 1224)
        );
        assert_eq!(place(DP1, s, Anchor::TopLeft, (24, 24)), (24, 1224));
        assert_eq!(
            place(DP1, s, Anchor::BottomLeft, (24, 24)),
            (24, 1200 + 1440 - 300 - 24)
        );
        assert_eq!(place(DP1, s, Anchor::BottomRight, (0, 0)), (4720, 2340));
        assert_eq!(place(DP1, s, Anchor::Center, (0, 0)), (2360, 1770));
        assert_eq!(place(DP1, s, Anchor::TopCenter, (10, 5)), (2370, 1205));
    }

    #[test]
    fn never_partly_outside() {
        let s = (400, 300);
        // negative or huge offsets are clamped to the monitor
        assert_eq!(place(DP1, s, Anchor::TopLeft, (-50, -80)), (0, 1200));
        assert_eq!(place(DP1, s, Anchor::TopLeft, (9000, 9000)), (4720, 2340));
        assert_eq!(place(DP1, s, Anchor::TopRight, (6000, 0)), (0, 1200));
        // monitor left of the primary (negative origin)
        assert_eq!(
            place((-1920, 0, 1920, 1080), s, Anchor::TopLeft, (-5, 0)),
            (-1920, 0)
        );
        // window larger than the monitor: top-left corner of the monitor
        assert_eq!(
            place((0, 0, 300, 200), s, Anchor::BottomRight, (0, 0)),
            (0, 0)
        );
    }

    #[test]
    fn offset_round_trip_and_clamp() {
        let s = (400, 300);
        for a in Anchor::ALL.map(Anchor::parse) {
            for off in [(24, 24), (0, 0), (300, 150), (-10, 40)] {
                let p = place(DP1, s, a, off);
                assert_eq!(
                    place(DP1, s, a, offset_for(DP1, s, a, p)),
                    p,
                    "{a:?} {off:?}"
                );
            }
        }
        // dragged past the right edge: stays on the monitor, offset 0 from the right
        assert_eq!(offset_for(DP1, s, Anchor::TopRight, (6000, 1300)), (0, 100));
        assert_eq!(offset_for(DP1, s, Anchor::TopLeft, (-50, 0)), (0, 0));
    }

    /// Second monitor of the same layout: DP-2 2560x1200 above DP-1.
    const DP2: (i32, i32, u32, u32) = (1280, 0, 2560, 1200);

    #[test]
    fn moved_object_keeps_offsets_and_monitor() {
        let mut o = MovedObject::new("DP-1", DP1, "top-right", (24, 24), (400, 300));
        assert_eq!(o.pos, (4696, 1224));
        assert_eq!(o.offset(), (24, 24));
        // dragged in the layer to an absolute position on DP-1
        o.set_pos((100, 1300));
        assert_eq!(o.offset(), (5120 - 400 - 100, 100));
        // never off the monitor (DP-1 starts at y=1200)
        o.set_pos((-500, 0));
        assert_eq!((o.pos, o.offset()), ((0, 1200), (4720, 0)));
        o.nudge(10, -10);
        assert_eq!(o.pos, (10, 1200));
        o.set_pos((9000, 9000));
        assert_eq!(o.pos, (4720, 2340));
        // to the other monitor: same anchor and offset there
        o.set_pos((5120 - 400 - 50, 1200 + 70));
        o.to_monitor("DP-2", DP2);
        assert_eq!((o.monitor.as_str(), o.offset()), ("DP-2", (50, 70)));
        assert_eq!(o.pos, (1280 + 2560 - 400 - 50, 70));
        o.reset("top-left", (24, 24));
        assert_eq!((o.pos, o.offset()), ((1304, 24), (24, 24)));
    }

    #[test]
    fn size_steps() {
        assert_eq!(size_factor("auto", 1080), 0.85);
        assert_eq!(size_factor("auto", 1440), 1.0);
        assert_eq!(size_factor("auto", 2160), 1.3);
        assert_eq!(size_factor("", 2880), 1.6);
        assert_eq!(size_factor("XL", 1080), 1.6);
        assert_eq!(Anchor::parse("nonsense"), Anchor::TopRight);
        let all: std::collections::HashSet<_> = Anchor::ALL
            .iter()
            .map(|a| format!("{:?}", Anchor::parse(a)))
            .collect();
        assert_eq!(all.len(), Anchor::ALL.len());
    }

    #[test]
    fn popup_window_only_while_a_popup_shows() {
        let mut g = PopupGate::default();
        assert!(!g.active(0), "no pop-up yet: window hidden");
        assert!(!popup_window_wanted(g.active(0), false));
        assert!(popup_window_wanted(false, true), "move mode shows it");
        let (first, until) = g.start(1_000, 4_000);
        assert_eq!(
            until,
            1_000 + 4_000 + POPUP_FADE_OUT_MS + POPUP_HIDE_SLACK_MS
        );
        assert!(g.active(2_000));
        // a focus change during the pop-up keeps it; after it, it stays hidden
        assert!(popup_window_wanted(g.active(until - 1), false));
        assert!(!popup_window_wanted(g.active(until), false));
        // a second pop-up (kill chain) extends; the first hide is ignored
        let (second, until2) = g.start(3_000, 4_000);
        assert!(until2 > until);
        assert!(!g.expire(first, until));
        assert!(!g.expire(second, until2 - 1));
        assert!(g.expire(second, until2));
        // the minimum display time applies
        let mut g = PopupGate::default();
        let (_, u) = g.start(0, 10);
        assert_eq!(u, POPUP_MIN_MS + POPUP_FADE_OUT_MS + POPUP_HIDE_SLACK_MS);
    }
}
