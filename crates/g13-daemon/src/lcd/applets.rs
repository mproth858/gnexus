//! The built-in applets, in BD cycle order: Profile, Profiles, Media,
//! Clock, System, Temperatures.

use chrono::{Datelike, Timelike};
use g13_core::{G13LcdBuffer, LCD_HEIGHT, LCD_WIDTH};

use super::{Applet, LcdAction, LcdContext};
use crate::platform::{MediaCommand, PlaybackStatus};

pub fn all() -> Vec<Box<dyn Applet>> {
    vec![Box::new(Status), Box::new(Profiles::default()), Box::new(Media), Box::new(Clock), Box::new(SystemMon), Box::new(Temperatures::default())]
}

/// Index of the profile list in [`all`].
pub const PROFILES: usize = 1;

const W: usize = LCD_WIDTH;
/// Top of the soft-key bar (L1–L4 labels).
const KEYS_Y: usize = 35;

/// Inverted 9-px header bar with `left` and `right` labels.
fn header(lcd: &mut G13LcdBuffer, left: &str, right: &str) {
    lcd.draw_text(3, 1, left);
    let rw = G13LcdBuffer::text_width(right, 1);
    lcd.draw_text(W.saturating_sub(rw + 3), 1, right);
    lcd.invert_rect(0, 0, W, 9);
}

fn centered(lcd: &mut G13LcdBuffer, y: usize, text: &str, scale: usize) {
    let w = G13LcdBuffer::text_width(text, scale).min(W);
    lcd.draw_text_scaled((W - w) / 2, y, text, scale);
}

/// Labels of L1–L4 in four 40-px cells at the bottom (the buttons sit under
/// the screen in the same order). Nothing is drawn when all are empty.
pub fn softkeys(lcd: &mut G13LcdBuffer, keys: [&str; 4]) {
    if keys.iter().all(|k| k.is_empty()) {
        return;
    }
    lcd.fill_rect(0, KEYS_Y - 1, W, LCD_HEIGHT - KEYS_Y + 1, false);
    lcd.draw_hline(0, KEYS_Y - 1, W);
    for (i, k) in keys.iter().enumerate() {
        if k.is_empty() {
            continue;
        }
        let cell = W / 4;
        let w = G13LcdBuffer::text_width(k, 1).min(cell);
        lcd.draw_text(i * cell + (cell - w) / 2, KEYS_Y + 1, k);
    }
}

/// One line of text, scrolling horizontally when wider than `width`.
fn marquee(lcd: &mut G13LcdBuffer, x: usize, y: usize, width: usize, text: &str, frame: u64) {
    let text = text.trim();
    let tw = G13LcdBuffer::text_width(text, 1);
    if tw <= width {
        lcd.draw_text(x, y, text);
    } else {
        // pause at the start, then scroll 3 px per frame, then loop
        let span = tw + 24;
        let step = (frame as usize * 3) % (span + 30);
        let offset = step.saturating_sub(30).min(span);
        let looped = format!("{text}    {text}");
        lcd.draw_text_window(x, y, width, offset, &looped);
    }
}

fn mmss(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

// ---- Profil (active profile) ---------------------------------------------------------

pub struct Status;

impl Applet for Status {
    fn title(&self) -> &'static str {
        "Profile"
    }

    fn keys(&self) -> [&'static str; 4] {
        ["", "", "PROFILES", ""]
    }

    fn on_key(&mut self, index: usize, _ctx: &LcdContext<'_>) -> LcdAction {
        if index == 2 { LcdAction::Goto(PROFILES) } else { LcdAction::None }
    }

    fn render(&mut self, lcd: &mut G13LcdBuffer, ctx: &LcdContext<'_>) {
        let left = if ctx.pinned.is_some() { "PROFILE - PINNED" } else { "PROFILE" };
        header(lcd, left, &format!("MENU {}", ctx.bank));
        let name = ctx.profile_name;
        if G13LcdBuffer::text_width(name, 2) <= W - 4 {
            centered(lcd, 12, name, 2);
        } else {
            marquee(lcd, 2, 15, W - 4, name, ctx.frame);
        }
        // the game only when it is what selected this profile
        let foot = match ctx.game {
            _ if ctx.pinned.is_some() => "Active everywhere".to_string(),
            Some(g) if ctx.game_matched && !g.is_empty() => g.to_string(),
            _ if ctx.profile_default => "Default profile".to_string(),
            _ => String::new(),
        };
        marquee(lcd, 2, 26, W - 4, &foot, ctx.frame);
    }
}

// ---- Profils (pick a profile, like LGS) ---------------------------------------------

/// "Auto" (automatic switching) followed by every profile; L1/L2 move,
/// L3 confirms. Choosing a profile pins it everywhere until "Auto".
#[derive(Default)]
pub struct Profiles {
    cursor: usize,
}

impl Profiles {
    /// Rows: `None` = Auto, then the profile ids.
    fn rows(ctx: &LcdContext<'_>) -> Vec<Option<String>> {
        std::iter::once(None).chain(ctx.profiles.iter().map(|p| Some(p.id.clone()))).collect()
    }
}

impl Applet for Profiles {
    fn title(&self) -> &'static str {
        "Profiles"
    }

    fn keys(&self) -> [&'static str; 4] {
        ["UP", "DOWN", "OK", ""]
    }

    fn on_show(&mut self, ctx: &LcdContext<'_>) {
        let rows = Self::rows(ctx);
        let want = ctx.pinned.map(str::to_string);
        self.cursor = rows.iter().position(|r| *r == want).unwrap_or(0);
    }

    fn on_key(&mut self, index: usize, ctx: &LcdContext<'_>) -> LcdAction {
        let rows = Self::rows(ctx);
        match index {
            0 => {
                self.cursor = self.cursor.checked_sub(1).unwrap_or(rows.len() - 1);
                LcdAction::None
            }
            1 => {
                self.cursor = (self.cursor + 1) % rows.len();
                LcdAction::None
            }
            2 => LcdAction::Pin(rows.get(self.cursor).cloned().flatten()),
            _ => LcdAction::None,
        }
    }

    fn render(&mut self, lcd: &mut G13LcdBuffer, ctx: &LcdContext<'_>) {
        header(lcd, "PROFILES", if ctx.pinned.is_some() { "PINNED" } else { "AUTO" });
        let rows = Self::rows(ctx);
        self.cursor = self.cursor.min(rows.len().saturating_sub(1));
        // 3 visible rows, scrolled so the cursor stays visible
        let first = self.cursor.saturating_sub(1).min(rows.len().saturating_sub(3));
        for (line, (i, row)) in rows.iter().enumerate().skip(first).take(3).enumerate() {
            let y = 11 + line * 8;
            let chosen = match row {
                None => ctx.pinned.is_none(),
                Some(id) => ctx.pinned == Some(id.as_str()),
            };
            let label = match row {
                None => "Auto (by game)".to_string(),
                Some(id) => {
                    let n = ctx.profiles.iter().position(|p| &p.id == id).map_or(0, |n| n + 1);
                    let name = ctx.profiles.iter().find(|p| &p.id == id).map_or(id.as_str(), |p| p.name.as_str());
                    let active = if id == ctx.active_id { " <" } else { "" };
                    format!("{n}. {name}{active}")
                }
            };
            lcd.draw_text(3, y, if chosen { "*" } else { " " });
            lcd.draw_text(11, y, &label);
            if i == self.cursor {
                lcd.invert_rect(0, y - 1, W, 9);
            }
        }
    }
}

// ---- Média ---------------------------------------------------------------------------

pub struct Media;

impl Applet for Media {
    fn title(&self) -> &'static str {
        "Media"
    }

    fn keys(&self) -> [&'static str; 4] {
        ["|<<", ">||", ">>|", ""]
    }

    fn wants_media(&self) -> bool {
        true
    }

    fn on_key(&mut self, index: usize, _ctx: &LcdContext<'_>) -> LcdAction {
        match index {
            0 => LcdAction::Media(MediaCommand::Previous),
            1 => LcdAction::Media(MediaCommand::PlayPause),
            2 => LcdAction::Media(MediaCommand::Next),
            _ => LcdAction::None,
        }
    }

    fn render(&mut self, lcd: &mut G13LcdBuffer, ctx: &LcdContext<'_>) {
        let Some(m) = &ctx.media else {
            header(lcd, "MEDIA", "");
            centered(lcd, 18, "No media playing", 1);
            return;
        };
        let state = match m.status {
            PlaybackStatus::Playing => "> PLAYING",
            PlaybackStatus::Paused => "|| PAUSED",
            PlaybackStatus::Stopped => "[] STOPPED",
        };
        let source = m.source.as_deref().map(short_source).unwrap_or_default();
        header(lcd, state, &source);
        marquee(lcd, 2, 11, W - 4, &m.title, ctx.frame);
        if let (Some(p), Some(d)) = (m.position_s, m.duration_s)
            && d > 0.0
        {
            // artist + times on one line, bar under it
            marquee(lcd, 2, 19, W - 56, &m.artist, ctx.frame);
            let t = format!("{}/{}", mmss(p), mmss(d));
            let tw = G13LcdBuffer::text_width(&t, 1);
            lcd.draw_text(W - tw - 1, 19, &t);
            lcd.draw_bar(2, 27, W - 4, 6, (p / d) as f32);
        } else {
            marquee(lcd, 2, 21, W - 4, &m.artist, ctx.frame);
        }
    }
}

/// "Spotify.exe" / "Chrome" / AUMID "SpotifyAB.SpotifyMusic_…!Spotify" → short name.
fn short_source(s: &str) -> String {
    let s = s.rsplit(['!', '\\', '/']).next().unwrap_or(s);
    let s = s.strip_suffix(".exe").unwrap_or(s);
    let s = s.split('_').next().unwrap_or(s);
    let s = s.rsplit('.').next().unwrap_or(s);
    s.chars().take(10).collect::<String>().to_uppercase()
}

// ---- Horloge -------------------------------------------------------------------------

pub struct Clock;

const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November",
    "December",
];

impl Applet for Clock {
    fn title(&self) -> &'static str {
        "Clock"
    }

    fn render(&mut self, lcd: &mut G13LcdBuffer, ctx: &LcdContext<'_>) {
        let t = ctx.now;
        let hm = format!("{:02}:{:02}", t.hour(), t.minute());
        let w = G13LcdBuffer::text_width(&hm, 4);
        let x = (W - w - 16) / 2;
        lcd.draw_text_scaled(x, 3, &hm, 4); // 28 px high
        lcd.draw_text(x + w + 4, 24, &format!("{:02}", t.second()));
        let date = format!(
            "{} {} {} {}",
            &DAYS[t.weekday().num_days_from_monday() as usize][..3],
            t.day(),
            &MONTHS[t.month0() as usize][..3],
            t.year()
        );
        centered(lcd, LCD_HEIGHT - 8, &date, 1);
    }
}

// ---- Système -------------------------------------------------------------------------

pub struct SystemMon;

impl Applet for SystemMon {
    fn title(&self) -> &'static str {
        "System"
    }

    fn render(&mut self, lcd: &mut G13LcdBuffer, ctx: &LcdContext<'_>) {
        let s = ctx.sys;
        let gb = |b: u64| b as f64 / 1_073_741_824.0;
        // the mouse battery when there is one, else the memory in GB
        let right = match ctx.mouse_battery {
            Some((pct, charging)) => format!("MOUSE {pct}%{}", if charging { "+" } else { "" }),
            None if s.mem_total > 0 => format!("{:.1}/{:.0} GB", gb(s.mem_used), gb(s.mem_total)),
            None => String::new(),
        };
        header(lcd, "SYSTEM", &right);
        lcd.draw_text(2, 14, "CPU");
        lcd.draw_bar(24, 13, 100, 9, s.cpu / 100.0);
        lcd.draw_text(130, 14, &format!("{:>3.0}%", s.cpu));
        let mem = if s.mem_total > 0 { s.mem_used as f32 / s.mem_total as f32 } else { 0.0 };
        lcd.draw_text(2, 29, "RAM");
        lcd.draw_bar(24, 28, 100, 9, mem);
        lcd.draw_text(130, 29, &format!("{:>3.0}%", mem * 100.0));
    }
}

// ---- Temperatures: CPU/GPU first, additional sensors in pairs -----------------

#[derive(Default)]
pub struct Temperatures { page: usize }

impl Temperatures {
    fn pages(ctx: &LcdContext<'_>) -> Vec<Vec<(String, Option<f32>)>> {
        use crate::platform::temperatures::{Kind, primary};
        let cpu = primary(&ctx.temperatures, Kind::Cpu);
        let gpu = primary(&ctx.temperatures, Kind::Gpu);
        let mut pages = vec![vec![("CPU".into(), cpu.map(|r| r.celsius)), ("GPU".into(), gpu.map(|r| r.celsius))]];
        let extra: Vec<_> = ctx.temperatures.iter().filter(|r| r.valid() && Some(r.id.as_str()) != cpu.map(|r| r.id.as_str()) && Some(r.id.as_str()) != gpu.map(|r| r.id.as_str()))
            .map(|r| {
                let label = match r.kind {
                    Kind::Cpu if r.name.starts_with("CPU") => r.name.clone(),
                    Kind::Cpu => format!("CPU {}", r.name),
                    Kind::Gpu if r.hardware.contains("Radeon(TM) Graphics") => format!("iGPU {}", r.name.strip_prefix("GPU ").unwrap_or(&r.name)),
                    Kind::Gpu if r.name.starts_with("GPU") || r.name.starts_with("iGPU") => r.name.clone(),
                    Kind::Gpu => format!("GPU {}", r.name),
                    Kind::Other => {
                        if let Some(index) = r.id.split_once("/nvme/").and_then(|(_, rest)| rest.split('/').next()).and_then(|index| index.parse::<u32>().ok()) {
                            format!("SSD {} {}", index.saturating_add(1), r.name.replace("Composite Temperature", "Composite").replace("Temperature #", "Sensor #"))
                        } else if r.name.starts_with("DIMM") { format!("RAM {}", r.name) }
                        else if r.name == "CPU" { "Board CPU".into() }
                        else { r.name.clone() }
                    }
                };
                (label, Some(r.celsius))
            }).collect();
        for pair in extra.chunks(2) { pages.push(pair.to_vec()); }
        pages
    }
}
impl Applet for Temperatures {
    fn title(&self) -> &'static str { "Temperatures" }
    fn wants_temperatures(&self) -> bool { true }
    fn keys(&self) -> [&'static str; 4] { ["PREV", "NEXT", "HOME", ""] }
    fn on_show(&mut self, _ctx: &LcdContext<'_>) { self.page = 0; }
    fn on_key(&mut self, index: usize, ctx: &LcdContext<'_>) -> LcdAction {
        let count = Self::pages(ctx).len();
        self.page = self.page.min(count - 1);
        match index { 0 => self.page = (self.page + count - 1) % count, 1 => self.page = (self.page + 1) % count, 2 => self.page = 0, _ => {} }
        LcdAction::None
    }
    fn render(&mut self, lcd: &mut G13LcdBuffer, ctx: &LcdContext<'_>) {
        let pages = Self::pages(ctx);
        self.page = self.page.min(pages.len() - 1);
        header(lcd, "TEMPERATURES", &format!("{}/{} C", self.page + 1, pages.len()));
        for (i, (label, value)) in pages[self.page].iter().enumerate() {
            let y = 12 + i * 12;
            marquee(lcd, 2, y, 111, label, ctx.frame);
            let text = value.map(|c| format!("{c:.1} C")).unwrap_or_else(|| "-- C".into());
            let width = G13LcdBuffer::text_width(&text, 1);
            lcd.draw_text(W.saturating_sub(width + 2), y, &text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcd::{ProfileItem, SysStats};
    use crate::platform::MediaInfo;
    use chrono::TimeZone;

    fn ctx<'a>(media: Option<MediaInfo>, pinned: Option<&'a str>) -> LcdContext<'a> {
        LcdContext {
            profile_name: "WARDOGS",
            profile_default: false,
            bank: "M2",
            game: Some("wardogs.exe"),
            game_matched: true,
            pinned,
            active_id: "wardogs",
            profiles: vec![
                ProfileItem { id: "demo".into(), name: "Default".into() },
                ProfileItem { id: "wardogs".into(), name: "WARDOGS".into() },
                ProfileItem { id: "fps".into(), name: "FPS".into() },
            ],
            now: chrono::Local.with_ymd_and_hms(2026, 10, 16, 21, 47, 5).unwrap(),
            media,
            temperatures: Vec::new(),
            sys: SysStats { cpu: 42.0, mem_used: 8 << 30, mem_total: 32 << 30 },
            mouse_battery: Some((93, true)),
            frame: 0,
        }
    }

    fn lit(lcd: &G13LcdBuffer) -> usize {
        lcd.pixels().iter().map(|b| b.count_ones() as usize).sum()
    }

    #[test]
    fn temperatures_follow_system_and_browse_optional_sensors() {
        use crate::platform::temperatures::{Kind, Reading};
        let names: Vec<_> = all().iter().map(|a| a.title()).collect();
        assert_eq!(&names[names.len() - 2..], &["System", "Temperatures"]);
        let mut context = ctx(None, None);
        context.temperatures = vec![
            Reading { id: "nvml:0".into(), name: "GPU Core".into(), hardware: "NVIDIA".into(), kind: Kind::Gpu, celsius: 60.0 },
            Reading { id: "hotspot".into(), name: "GPU Hot Spot".into(), hardware: "NVIDIA".into(), kind: Kind::Gpu, celsius: 75.0 },
            Reading { id: "ssd".into(), name: "Composite".into(), hardware: "SSD".into(), kind: Kind::Other, celsius: 42.0 },
        ];
        let pages = Temperatures::pages(&context);
        assert_eq!(pages[0], vec![("CPU".into(), None), ("GPU".into(), Some(60.0))]);
        assert_eq!(pages.len(), 2);
        let mut applet = Temperatures::default();
        applet.on_key(1, &context);
        assert_eq!(applet.page, 1);
        applet.on_key(1, &context);
        assert_eq!(applet.page, 0);
        applet.on_key(0, &context);
        assert_eq!(applet.page, 1);
        applet.on_key(2, &context);
        assert_eq!(applet.page, 0);
        // A lost provider removes extra pages without invalid indexes or panic.
        applet.on_key(1, &context);
        context.temperatures.clear();
        applet.render(&mut G13LcdBuffer::new(), &context);
        assert_eq!(applet.page, 0);
    }

    #[test]
    fn every_applet_draws_something() {
        let media = MediaInfo {
            title: "Une chanson très longue qui doit défiler sur l'écran".into(),
            artist: "Quelqu'un".into(),
            album: None,
            status: PlaybackStatus::Playing,
            position_s: Some(83.0),
            duration_s: Some(225.0),
            source: Some("Spotify.exe".into()),
        };
        for mut a in all() {
            for m in [Some(media.clone()), None] {
                let mut lcd = G13LcdBuffer::new();
                a.render(&mut lcd, &ctx(m, None));
                softkeys(&mut lcd, a.keys());
                assert!(lit(&lcd) > 50, "{} drew almost nothing", a.title());
            }
        }
    }

    #[test]
    fn profile_list_navigation() {
        let mut p = Profiles::default();
        let c = ctx(None, None);
        p.on_show(&c);
        assert_eq!(p.cursor, 0, "starts on Auto when nothing is pinned");
        assert_eq!(p.on_key(0, &c), LcdAction::None); // up wraps to the last profile
        assert_eq!(p.on_key(2, &c), LcdAction::Pin(Some("fps".into())));
        p.on_key(1, &c); // down wraps to Auto
        assert_eq!(p.on_key(2, &c), LcdAction::Pin(None));
        // shown again while "wardogs" is pinned: cursor on it
        p.on_show(&ctx(None, Some("wardogs")));
        assert_eq!(p.on_key(2, &c), LcdAction::Pin(Some("wardogs".into())));
    }

    #[test]
    fn source_names_are_shortened() {
        assert_eq!(short_source("Spotify.exe"), "SPOTIFY");
        assert_eq!(short_source("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "SPOTIFY");
        assert_eq!(short_source("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "ZUNEMUSIC");
    }
}
