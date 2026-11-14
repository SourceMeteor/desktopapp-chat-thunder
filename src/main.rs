#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod firebase;
mod model;

use eframe::egui::*;
use firebase::{inc, room_id, server_ts, Db, Res, Session};
use model::*;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/* ====================== PALETA ====================== */

#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    side: Color32,
    bar: Color32,
    panel: Color32,
    input: Color32,
    hover: Color32,
    border: Color32,
    text: Color32,
    text2: Color32,
    text3: Color32,
    accent: Color32,
    gold: Color32,
    danger: Color32,
    good: Color32,
}
impl Palette {
    fn dark() -> Self {
        Palette {
            bg: Color32::from_rgb(9, 11, 22),
            side: Color32::from_rgb(11, 14, 27),
            bar: Color32::from_rgb(7, 9, 17),
            panel: Color32::from_rgb(15, 20, 38),
            input: Color32::from_rgb(21, 27, 51),
            hover: Color32::from_rgb(27, 35, 66),
            border: Color32::from_rgb(38, 46, 80),
            text: Color32::from_rgb(238, 241, 251),
            text2: Color32::from_rgb(139, 147, 173),
            text3: Color32::from_rgb(90, 98, 128),
            accent: Color32::from_rgb(0, 212, 255),
            gold: Color32::from_rgb(255, 234, 0),
            danger: Color32::from_rgb(255, 92, 114),
            good: Color32::from_rgb(52, 211, 153),
        }
    }
    fn light() -> Self {
        Palette {
            bg: Color32::from_rgb(238, 241, 250),
            side: Color32::from_rgb(230, 234, 246),
            bar: Color32::from_rgb(222, 227, 242),
            panel: Color32::from_rgb(255, 255, 255),
            input: Color32::from_rgb(240, 242, 250),
            hover: Color32::from_rgb(226, 231, 245),
            border: Color32::from_rgb(205, 211, 230),
            text: Color32::from_rgb(19, 23, 41),
            text2: Color32::from_rgb(75, 83, 114),
            text3: Color32::from_rgb(125, 133, 165),
            accent: Color32::from_rgb(0, 131, 173),
            gold: Color32::from_rgb(185, 138, 0),
            danger: Color32::from_rgb(225, 29, 72),
            good: Color32::from_rgb(13, 150, 104),
        }
    }
}

fn tint(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn apply_theme(ctx: &Context, p: &Palette, dark: bool) {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.input;
    v.faint_bg_color = p.side;
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.window_rounding = Rounding::same(14.0);
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = tint(p.accent, 70);
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
        w.rounding = Rounding::same(8.0);
    }
    v.widgets.inactive.weak_bg_fill = p.input;
    v.widgets.inactive.bg_fill = p.input;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    ctx.set_visuals(v);
}

fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match (i as i32).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn frame_color(frame: &str, t: f64) -> Option<Color32> {
    let f = frame.replace('-', "_");
    Some(match f.as_str() {
        "frame_neon" => Color32::from_rgb(0, 212, 255),
        "frame_fire" => Color32::from_rgb(255, 122, 59),
        "frame_gold" => Color32::from_rgb(255, 215, 94),
        "frame_ice" => Color32::from_rgb(140, 231, 255),
        "frame_void" => Color32::from_rgb(179, 138, 255),
        "frame_thunder" => Color32::from_rgb(255, 231, 94),
        "frame_rainbow" => hsv(((t * 0.3) % 1.0) as f32, 0.7, 1.0),
        _ => return None,
    })
}

fn nick_color(ui: &Ui, gold: bool, rainbow: bool, fx: &str, p: &Palette) -> Color32 {
    if rainbow || fx == "rainbow_nick" {
        ui.ctx().request_repaint_after(Duration::from_millis(80));
        return hsv(((ui.input(|i| i.time) * 0.25) % 1.0) as f32, 0.65, 1.0);
    }
    if gold || fx == "gold_nick" {
        return Color32::from_rgb(255, 215, 70);
    }
    match fx {
        "frost_nick" => Color32::from_rgb(140, 220, 255),
        "plasma_nick" => Color32::from_rgb(215, 110, 255),
        _ => p.accent,
    }
}

fn avatar(ui: &mut Ui, url: &str, nick: &str, size: f32, ring: Option<Color32>) -> Response {
    let resp = if url.starts_with("http") {
        ui.add(
            Image::new(url.to_string())
                .fit_to_exact_size(vec2(size, size))
                .rounding(size / 2.0)
                .sense(Sense::click()),
        )
    } else {
        let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
        let mut h: u32 = 7;
        for b in nick.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as u32);
        }
        let col = hsv((h % 360) as f32 / 360.0, 0.55, 0.75);
        ui.painter().circle_filled(rect.center(), size / 2.0, col);
        let ch = nick.chars().next().unwrap_or('?').to_uppercase().to_string();
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            ch,
            FontId::proportional(size * 0.5),
            Color32::WHITE,
        );
        resp
    };
    if let Some(c) = ring {
        ui.painter()
            .circle_stroke(resp.rect.center(), size / 2.0 + 1.5, Stroke::new(2.0_f32, c));
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/* ====================== PASEK TYTUŁU ====================== */

#[derive(Clone, Copy, PartialEq)]
enum WinIcon {
    Back,
    Fwd,
    Min,
    Max,
    Restore,
    Close,
}

fn win_icon(ui: &mut Ui, icon: WinIcon, w: f32, enabled: bool, p: &Palette) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(w, 36.0),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let hov = enabled && resp.hovered();
    let is_close = icon == WinIcon::Close;
    if hov {
        ui.painter()
            .rect_filled(rect, 0.0, if is_close { p.danger } else { p.hover });
    }
    let col = if !enabled {
        p.text3.gamma_multiply(0.45)
    } else if hov && is_close {
        Color32::WHITE
    } else if hov {
        p.text
    } else {
        p.text2
    };
    let st = Stroke::new(1.3_f32, col);
    let c = rect.center();
    let s = 5.0;
    let pt = |dx: f32, dy: f32| pos2(c.x + dx, c.y + dy);
    let pa = ui.painter();
    match icon {
        WinIcon::Min => {
            pa.line_segment([pt(-s, 0.0), pt(s, 0.0)], st);
        }
        WinIcon::Max => {
            pa.rect_stroke(Rect::from_center_size(c, vec2(2.0 * s, 2.0 * s)), 1.0, st);
        }
        WinIcon::Restore => {
            pa.rect_stroke(
                Rect::from_min_size(pt(-s, -s + 2.5), vec2(2.0 * s - 2.5, 2.0 * s - 2.5)),
                1.0,
                st,
            );
            pa.line_segment([pt(-s + 2.5, -s + 2.5), pt(-s + 2.5, -s)], st);
            pa.line_segment([pt(-s + 2.5, -s), pt(s, -s)], st);
            pa.line_segment([pt(s, -s), pt(s, s - 2.5)], st);
        }
        WinIcon::Close => {
            pa.line_segment([pt(-s, -s), pt(s, s)], st);
            pa.line_segment([pt(-s, s), pt(s, -s)], st);
        }
        WinIcon::Back => {
            pa.line_segment([pt(s, 0.0), pt(-s, 0.0)], st);
            pa.line_segment([pt(-s, 0.0), pt(-s + 4.5, -4.5)], st);
            pa.line_segment([pt(-s, 0.0), pt(-s + 4.5, 4.5)], st);
        }
        WinIcon::Fwd => {
            pa.line_segment([pt(-s, 0.0), pt(s, 0.0)], st);
            pa.line_segment([pt(s, 0.0), pt(s - 4.5, -4.5)], st);
            pa.line_segment([pt(s, 0.0), pt(s - 4.5, 4.5)], st);
        }
    }
    resp
}

fn nav_row(ui: &mut Ui, icon: &str, label: &str, active: bool, p: &Palette) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let bg = if active {
        tint(p.accent, 36)
    } else if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 10.0, bg);
    if active {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + vec2(0.0, 9.0), vec2(3.0, 20.0)),
            2.0,
            p.accent,
        );
    }
    let col = if active { p.text } else { p.text2 };
    ui.painter().text(
        rect.left_center() + vec2(18.0, 0.0),
        Align2::LEFT_CENTER,
        icon,
        FontId::proportional(16.0),
        if active { p.accent } else { col },
    );
    ui.painter().text(
        rect.left_center() + vec2(46.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        col,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::none()
        .fill(p.panel)
        .rounding(16.0)
        .stroke(Stroke::new(1.0_f32, p.border))
        .inner_margin(Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/* ====================== STAN APLIKACJI ====================== */

#[derive(Clone, PartialEq)]
enum View {
    Chat,
    Friends,
    Shop,
    Settings,
    Dm(String),
}

struct Reply {
    id: String,
    uid: String,
    nick: String,
    text: String,
}
struct Chan {
    msgs: Vec<ChatMsg>,
    last_full: Option<Instant>,
}
struct Friend {
    uid: String,
    user: Value,
    last: Value,
}

enum Ev {
    Auth(Res<Session>),
    Registered(Res<()>),
    User(Value),
    Chat { path: String, res: Res<Vec<ChatMsg>>, full: bool },
    Sent,
    Friends(Vec<Friend>),
    Presence(Value),
    MyRx(Vec<String>),
    Profile(String, Value),
    Toast(String),
    RefreshFriends,
    RefreshUser,
}

enum Act {
    React(String, String, &'static str),
    Reply(ChatMsg),
    Edit(ChatMsg),
    Delete(String),
    Copy(String),
    Profile(String),
}

enum ShopAct {
    Buy(usize),
    Equip(usize),
    Unequip(usize),
}

struct App {
    ctx: Context,
    p: Palette,
    dark: bool,
    db: Db,
    tx: Sender<Ev>,
    rx: Receiver<Ev>,
    logged: bool,
    uid: String,
    email: String,
    // logowanie
    auth_tab: u8,
    a_email: String,
    a_pass: String,
    a_pass2: String,
    a_nick: String,
    a_err: String,
    a_info: String,
    a_busy: bool,
    // nawigacja
    hist: Vec<View>,
    hist_i: usize,
    nav_at: Instant,
    // dane
    user: Value,
    prev_level: i64,
    daily_checked: bool,
    daily: Option<(i64, i64)>,
    chats: HashMap<String, Chan>,
    chat_inflight: bool,
    last_chat_poll: Instant,
    poll_now: bool,
    friends: Vec<Friend>,
    presence: Value,
    my_rx: HashSet<String>,
    last_user: Instant,
    last_friends: Instant,
    last_presence: Instant,
    last_err: Instant,
    offline_sent: bool,
    // wpisywanie
    input: String,
    reply: Option<Reply>,
    edit: Option<(String, String)>,
    // UI
    shop_cat: usize,
    buy: Option<usize>,
    profile: Option<(String, Value)>,
    show_online: bool,
    toast: Option<(String, Instant)>,
    nick_in: String,
    avatar_in: String,
    status_in: String,
    settings_loaded: bool,
    add_friend_open: bool,
    add_friend_in: String,
    wipe_saved: bool,
}

fn ago(secs: u64) -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(secs))
        .unwrap_or_else(Instant::now)
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let st = cc.storage;
        let get = |k: &str| st.and_then(|s| s.get_string(k)).unwrap_or_default();
        let dark = get("dark") != "0";
        let rt = get("rt");
        let email = get("email");
        let p = if dark { Palette::dark() } else { Palette::light() };
        apply_theme(&cc.egui_ctx, &p, dark);
        let (tx, rx) = channel();
        let mut app = App {
            ctx: cc.egui_ctx.clone(),
            p,
            dark,
            db: Db::new(Session::default()),
            tx,
            rx,
            logged: false,
            uid: String::new(),
            email: email.clone(),
            auth_tab: 0,
            a_email: email,
            a_pass: String::new(),
            a_pass2: String::new(),
            a_nick: String::new(),
            a_err: String::new(),
            a_info: String::new(),
            a_busy: false,
            hist: vec![View::Chat],
            hist_i: 0,
            nav_at: ago(5),
            user: Value::Null,
            prev_level: 0,
            daily_checked: false,
            daily: None,
            chats: HashMap::new(),
            chat_inflight: false,
            last_chat_poll: ago(10),
            poll_now: true,
            friends: vec![],
            presence: Value::Null,
            my_rx: HashSet::new(),
            last_user: Instant::now(),
            last_friends: Instant::now(),
            last_presence: Instant::now(),
            last_err: ago(60),
            offline_sent: false,
            input: String::new(),
            reply: None,
            edit: None,
            shop_cat: 0,
            buy: None,
            profile: None,
            show_online: false,
            toast: None,
            nick_in: String::new(),
            avatar_in: String::new(),
            status_in: String::new(),
            settings_loaded: false,
            add_friend_open: false,
            add_friend_in: String::new(),
            wipe_saved: false,
        };
        if !rt.is_empty() {
            app.a_busy = true;
            app.bg(move || Some(Ev::Auth(firebase::refresh(&rt))));
        }
        app
    }

    /* ---------- pomocnicze ---------- */

    fn bg(&self, f: impl FnOnce() -> Option<Ev> + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            if let Some(ev) = f() {
                let _ = tx.send(ev);
            }
            ctx.request_repaint();
        });
    }
    fn say(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }
    fn view(&self) -> &View {
        &self.hist[self.hist_i]
    }
    fn current_path(&self) -> Option<String> {
        match self.view() {
            View::Chat => Some("messages".to_string()),
            View::Dm(u) => Some(format!("dms/{}/messages", room_id(&self.uid, u))),
            _ => None,
        }
    }
    fn title(&self) -> (String, String) {
        if !self.logged {
            return ("⚡".into(), "Thunder Chat".into());
        }
        match self.view() {
            View::Chat => ("💬".into(), "Czat globalny".into()),
            View::Friends => ("👥".into(), "Znajomi".into()),
            View::Shop => ("🛒".into(), "Sklep".into()),
            View::Settings => ("⚙".into(), "Ustawienia".into()),
            View::Dm(u) => {
                let nick = self
                    .friends
                    .iter()
                    .find(|f| &f.uid == u)
                    .map(|f| s(&f.user, "nick"))
                    .unwrap_or_else(|| "Wiadomość".into());
                ("@".into(), nick)
            }
        }
    }
    fn go(&mut self, v: View) {
        if *self.view() == v {
            return;
        }
        self.hist.truncate(self.hist_i + 1);
        self.hist.push(v);
        self.hist_i += 1;
        self.after_nav();
    }
    fn back(&mut self) {
        if self.hist_i > 0 {
            self.hist_i -= 1;
            self.after_nav();
        }
    }
    fn forward(&mut self) {
        if self.hist_i + 1 < self.hist.len() {
            self.hist_i += 1;
            self.after_nav();
        }
    }
    fn after_nav(&mut self) {
        self.nav_at = Instant::now();
        self.reply = None;
        self.edit = None;
        self.input.clear();
        self.poll_now = true;
        if let View::Dm(u) = self.view().clone() {
            let room = room_id(&self.uid, &u);
            self.fetch_rx(format!("dms/{room}/messages"), format!("dmReactions/{}/{room}", self.uid));
        }
    }

    /* ---------- sieć ---------- */

    fn bootstrap(&mut self) {
        self.fetch_user();
        self.fetch_friends();
        self.fetch_presence();
        self.fetch_rx("messages".into(), format!("reactions/{}", self.uid));
        let (db, uid) = (self.db.clone(), self.uid.clone());
        self.bg(move || {
            let _ = db.patch(
                &format!("presence/{uid}"),
                &json!({"online": true, "lastSeen": server_ts()}),
            );
            None
        });
    }
    fn fetch_user(&mut self) {
        self.last_user = Instant::now();
        let (db, uid) = (self.db.clone(), self.uid.clone());
        self.bg(move || {
            Some(match db.get(&format!("users/{uid}"), &[]) {
                Ok(v) => Ev::User(v),
                Err(e) => Ev::Toast(e),
            })
        });
    }
    fn fetch_friends(&mut self) {
        self.last_friends = Instant::now();
        let (db, me) = (self.db.clone(), self.uid.clone());
        self.bg(move || {
            let f = db.get(&format!("friends/{me}"), &[]).ok()?;
            let uids: Vec<String> = f
                .as_object()
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default();
            let mut out = vec![];
            for u in uids {
                let user = db.get(&format!("users/{u}"), &[]).unwrap_or(Value::Null);
                let room = room_id(&me, &u);
                let last = db
                    .get(&format!("dmRooms/{room}/lastMessage"), &[])
                    .unwrap_or(Value::Null);
                out.push(Friend { uid: u, user, last });
            }
            Some(Ev::Friends(out))
        });
    }
    fn fetch_presence(&mut self) {
        self.last_presence = Instant::now();
        let db = self.db.clone();
        self.bg(move || db.get("presence", &[]).ok().map(Ev::Presence));
    }
    fn fetch_rx(&self, scope: String, db_path: String) {
        let db = self.db.clone();
        self.bg(move || {
            let v = db.get(&db_path, &[]).ok()?;
            let mut keys = vec![];
            if let Some(o) = v.as_object() {
                for (id, types) in o {
                    if let Some(t) = types.as_object() {
                        for ty in t.keys() {
                            keys.push(format!("{scope}|{id}|{ty}"));
                        }
                    }
                }
            }
            Some(Ev::MyRx(keys))
        });
    }
    fn open_profile(&self, uid: String) {
        let db = self.db.clone();
        self.bg(move || {
            Some(match db.get(&format!("users/{uid}"), &[]) {
                Ok(v) => Ev::Profile(uid, v),
                Err(e) => Ev::Toast(e),
            })
        });
    }

    fn tick(&mut self) {
        if !self.logged {
            return;
        }
        let now = Instant::now();
        if let Some(path) = self.current_path() {
            let due = self.poll_now || now.duration_since(self.last_chat_poll) >= Duration::from_secs(2);
            if due && !self.chat_inflight {
                self.poll_now = false;
                self.chat_inflight = true;
                self.last_chat_poll = now;
                let chan = self.chats.get(&path);
                let stale = chan
                    .and_then(|c| c.last_full)
                    .map(|t| now.duration_since(t) > Duration::from_secs(20))
                    .unwrap_or(true);
                let after = if stale {
                    None
                } else {
                    chan.and_then(|c| c.msgs.last().map(|m| m.id.clone()))
                };
                let full = after.is_none();
                let db = self.db.clone();
                self.bg(move || {
                    let res = poll_chat(&db, &path, after);
                    Some(Ev::Chat { path, res, full })
                });
            }
        }
        if now.duration_since(self.last_user) > Duration::from_secs(10) {
            self.fetch_user();
        }
        if now.duration_since(self.last_friends) > Duration::from_secs(15) {
            self.fetch_friends();
        }
        if now.duration_since(self.last_presence) > Duration::from_secs(20) {
            self.fetch_presence();
        }
    }

    fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::Auth(Ok(sess)) => {
                *self.db.session.lock().unwrap() = sess.clone();
                self.uid = sess.uid.clone();
                if !sess.email.is_empty() {
                    self.email = sess.email.clone();
                } else if !self.a_email.is_empty() {
                    self.email = self.a_email.clone();
                }
                self.logged = true;
                self.a_busy = false;
                self.a_err.clear();
                self.a_pass.clear();
                self.hist = vec![View::Chat];
                self.hist_i = 0;
                self.offline_sent = false;
                self.wipe_saved = false;
                self.bootstrap();
            }
            Ev::Auth(Err(e)) => {
                self.a_busy = false;
                self.a_err = e;
                self.wipe_saved = true;
            }
            Ev::Registered(r) => {
                self.a_busy = false;
                match r {
                    Ok(()) => {
                        self.a_info = "Konto utworzone! Kliknij link weryfikacyjny w e-mailu, potem się zaloguj.".into();
                        self.auth_tab = 0;
                    }
                    Err(e) => self.a_err = e,
                }
            }
            Ev::User(v) => self.on_user(v),
            Ev::Chat { path, res, full } => {
                self.chat_inflight = false;
                match res {
                    Err(e) => {
                        if self.last_err.elapsed() > Duration::from_secs(10) {
                            self.last_err = Instant::now();
                            self.say(e);
                        }
                    }
                    Ok(msgs) => {
                        let ch = self.chats.entry(path).or_insert(Chan { msgs: vec![], last_full: None });
                        if full {
                            ch.msgs = msgs;
                            ch.last_full = Some(Instant::now());
                        } else {
                            for m in msgs {
                                if let Some(x) = ch.msgs.iter_mut().find(|x| x.id == m.id) {
                                    *x = m;
                                } else {
                                    ch.msgs.push(m);
                                }
                            }
                            ch.msgs.sort_by(|a, b| a.id.cmp(&b.id));
                        }
                    }
                }
            }
            Ev::Sent => self.poll_now = true,
            Ev::Friends(f) => self.friends = f,
            Ev::Presence(v) => self.presence = v,
            Ev::MyRx(keys) => self.my_rx.extend(keys),
            Ev::Profile(uid, v) => self.profile = Some((uid, v)),
            Ev::Toast(t) => self.say(t),
            Ev::RefreshFriends => self.fetch_friends(),
            Ev::RefreshUser => self.fetch_user(),
        }
    }

    fn on_user(&mut self, v: Value) {
        if v.is_null() {
            // konto bez profilu — utwórz domyślny
            let nick = self.email.split('@').next().unwrap_or("User").to_string();
            let (db, uid) = (self.db.clone(), self.uid.clone());
            self.bg(move || {
                let _ = db.patch(
                    &format!("users/{uid}"),
                    &json!({"nick": nick, "nickLower": nick.to_lowercase(), "avatar": "", "status": "",
                            "coins": 100, "xp": 0, "msgCount": 0, "heartsReceived": 0,
                            "mediaCount": 0, "createdAt": server_ts(), "friendCount": 0}),
                );
                Some(Ev::RefreshUser)
            });
            return;
        }
        let lvl = level_from_xp(n(&v, "xp"));
        if self.prev_level > 0 && lvl > self.prev_level {
            self.say(format!("⚡ Awans! Masz teraz poziom {lvl}"));
        }
        self.prev_level = lvl;
        self.user = v;
        if !self.settings_loaded {
            self.settings_loaded = true;
            self.nick_in = s(&self.user, "nick");
            self.avatar_in = s(&self.user, "avatar");
            self.status_in = s(&self.user, "status");
        }
        if !self.daily_checked {
            self.daily_checked = true;
            let last = s(&self.user, "lastDaily");
            if last != today_str() {
                let streak = if last == yesterday_str() { n(&self.user, "streak") + 1 } else { 1 };
                self.daily = Some((streak, (15 + streak * 5).min(60)));
            }
        }
    }

    fn base_fields(&self) -> Value {
        let u = &self.user;
        let mut frame = s(u, "activeFrame");
        if frame.is_empty() && owns(u, "frame_fire") {
            frame = "frame_fire".into();
        }
        if frame.is_empty() && owns(u, "frame_neon") {
            frame = "frame_neon".into();
        }
        let mut fx = s(u, "activeNameFx");
        if fx.is_empty() && owns(u, "rainbow_nick") {
            fx = "rainbow_nick".into();
        }
        if fx.is_empty() && owns(u, "gold_nick") {
            fx = "gold_nick".into();
        }
        json!({
            "nick": s(u, "nick"), "avatar": s(u, "avatar"), "uid": self.uid,
            "goldNick": fx == "gold_nick", "rainbowNick": fx == "rainbow_nick",
            "nameFx": if fx == "frost_nick" || fx == "plasma_nick" { fx.clone() } else { String::new() },
            "isVip": owns(u, "vip"), "legend": owns(u, "legend"), "frame": frame,
            "aura": s(u, "aura"), "level": level_from_xp(n(u, "xp")),
            "reactions": {"heart": 0, "funny": 0, "fire": 0}, "time": server_ts(),
        })
    }

    fn send(&mut self) {
        let Some(path) = self.current_path() else { return };
        let raw = self.input.trim().to_string();
        if raw.is_empty() {
            return;
        }
        let text = sanitize(&raw);
        self.input.clear();
        let db = self.db.clone();

        if let Some((ep, id)) = self.edit.take() {
            if let Some(c) = self.chats.get_mut(&ep) {
                c.last_full = None; // wymuś pełne odświeżenie
            }
            self.bg(move || {
                Some(
                    match db.patch(
                        &format!("{ep}/{id}"),
                        &json!({"text": text, "editedAt": server_ts()}),
                    ) {
                        Ok(_) => Ev::Sent,
                        Err(e) => Ev::Toast(e),
                    },
                )
            });
            return;
        }

        let mut f = self.base_fields();
        f["type"] = json!("text");
        f["text"] = json!(text.clone());
        if let Some(r) = self.reply.take() {
            f["replyTo"] = json!({"id": r.id, "uid": r.uid, "nick": r.nick, "text": r.text});
        }
        let inv = &self.user["inventory"];
        let coin = if n(inv, "boostUntil") > now_ms() { 30 } else { 15 };
        let xp = if n(inv, "xpBoostUntil") > now_ms() { 12 } else { 6 };
        let rewards = json!({"coins": inc(coin), "xp": inc(xp), "msgCount": inc(1)});
        let uid = self.uid.clone();
        let peer = match self.view() {
            View::Dm(u) => Some(u.clone()),
            _ => None,
        };
        self.bg(move || {
            match db.post(&path, &f) {
                Ok(_) => {
                    if let Some(peer) = peer {
                        let room = room_id(&uid, &peer);
                        let _ = db.patch(
                            &format!("dmRooms/{room}"),
                            &json!({
                                "lastMessage": {"text": text, "type": "text", "at": now_ms(), "fromUid": uid},
                                "members": {uid.clone(): true, peer: true}
                            }),
                        );
                    }
                    let _ = db.patch(&format!("users/{uid}"), &rewards);
                    Some(Ev::Sent)
                }
                Err(e) => Some(Ev::Toast(e)),
            }
        });
        self.fetch_user();
    }

    fn react(&mut self, path: &str, id: &str, owner: &str, ty: &'static str) {
        let key = format!("{path}|{id}|{ty}");
        if self.my_rx.contains(&key) {
            return;
        }
        self.my_rx.insert(key);
        if let Some(c) = self.chats.get_mut(path) {
            if let Some(m) = c.msgs.iter_mut().find(|m| m.id == id) {
                match ty {
                    "heart" => m.heart += 1,
                    "funny" => m.funny += 1,
                    _ => m.fire += 1,
                }
            }
        }
        let (mine, base) = if path == "messages" {
            (format!("reactions/{}/{id}/{ty}", self.uid), format!("messages/{id}"))
        } else {
            let room = path.split('/').nth(1).unwrap_or("").to_string();
            (
                format!("dmReactions/{}/{room}/{id}/{ty}", self.uid),
                format!("{path}/{id}"),
            )
        };
        let mut up = serde_json::Map::new();
        up.insert(mine, json!(true));
        up.insert(format!("{base}/reactions/{ty}"), inc(1));
        if ty == "heart" && owner != self.uid {
            up.insert(format!("users/{owner}/heartsReceived"), inc(1));
        }
        let db = self.db.clone();
        self.bg(move || db.patch("", &Value::Object(up)).err().map(Ev::Toast));
    }

    fn apply(&mut self, a: Act, path: &str) {
        match a {
            Act::React(id, owner, ty) => self.react(path, &id, &owner, ty),
            Act::Reply(m) => {
                self.edit = None;
                self.reply = Some(Reply { id: m.id, uid: m.uid, nick: m.nick, text: snippet(&m.text) });
            }
            Act::Edit(m) => {
                self.reply = None;
                self.edit = Some((path.to_string(), m.id));
                self.input = m.text;
            }
            Act::Delete(id) => {
                if let Some(c) = self.chats.get_mut(path) {
                    c.msgs.retain(|m| m.id != id);
                }
                let db = self.db.clone();
                let p = format!("{path}/{id}");
                self.bg(move || {
                    Some(match db.delete(&p) {
                        Ok(_) => Ev::Toast("Wiadomość usunięta".into()),
                        Err(e) => Ev::Toast(e),
                    })
                });
            }
            Act::Copy(t) => {
                self.ctx.copy_text(t);
                self.say("Skopiowano!");
            }
            Act::Profile(u) => self.open_profile(u),
        }
    }

    fn claim_daily(&mut self, streak: i64, coins: i64) {
        self.daily = None;
        let (db, uid) = (self.db.clone(), self.uid.clone());
        self.bg(move || {
            let r = db.patch(
                &format!("users/{uid}"),
                &json!({"streak": streak, "lastDaily": today_str(),
                        "coins": inc(coins), "xp": inc(25)}),
            );
            Some(match r {
                Ok(_) => Ev::RefreshUser,
                Err(e) => Ev::Toast(e),
            })
        });
        self.say(format!("Odebrano nagrodę: +{coins} 🪙 +25 XP"));
    }

    fn do_buy(&mut self, idx: usize) {
        let item = &ITEMS[idx];
        if n(&self.user, "coins") < item.price {
            self.say("Masz za mało monet!");
            return;
        }
        let uid = self.uid.clone();
        let mut up = serde_json::Map::new();
        up.insert(format!("users/{uid}/coins"), inc(-item.price));
        match item.kind {
            Kind::Toggle => {
                up.insert(format!("users/{uid}/inventory/{}", item.key), json!(true));
            }
            Kind::Equip(field, value) => {
                up.insert(format!("users/{uid}/inventory/{}", item.key), json!(true));
                up.insert(format!("users/{uid}/{field}"), json!(value));
            }
            Kind::Boost(field) => {
                let base = n(&self.user["inventory"], field).max(now_ms());
                up.insert(format!("users/{uid}/inventory/{field}"), json!(base + 3_600_000));
            }
        }
        let db = self.db.clone();
        self.bg(move || {
            Some(match db.patch("", &Value::Object(up)) {
                Ok(_) => Ev::RefreshUser,
                Err(e) => Ev::Toast(e),
            })
        });
        self.say("Kupiono! 🛍");
    }

    fn set_field(&mut self, field: &'static str, value: &'static str) {
        let (db, uid) = (self.db.clone(), self.uid.clone());
        self.bg(move || {
            Some(match db.patch(&format!("users/{uid}"), &json!({ field: value })) {
                Ok(_) => Ev::RefreshUser,
                Err(e) => Ev::Toast(e),
            })
        });
    }

    fn add_friend(&mut self, nick: String) {
        let nick = nick.trim().to_lowercase();
        if nick.is_empty() {
            return;
        }
        let (db, me) = (self.db.clone(), self.uid.clone());
        self.bg(move || {
            let q = format!("\"{nick}\"");
            let found = match db.get("users", &[("orderBy", "\"nickLower\""), ("equalTo", q.as_str())]) {
                Ok(v) => v,
                Err(e) => return Some(Ev::Toast(e)),
            };
            let Some((fuid, _)) = found.as_object().and_then(|o| o.iter().next()) else {
                return Some(Ev::Toast("Nie znaleziono użytkownika o takim nicku.".into()));
            };
            if *fuid == me {
                return Some(Ev::Toast("Nie możesz dodać samego siebie.".into()));
            }
            let room = room_id(&me, fuid);
            let mut up = serde_json::Map::new();
            up.insert(format!("friends/{me}/{fuid}"), json!(true));
            up.insert(format!("friends/{fuid}/{me}"), json!(true));
            up.insert(format!("dmRooms/{room}/members/{me}"), json!(true));
            up.insert(format!("dmRooms/{room}/members/{fuid}"), json!(true));
            up.insert(format!("users/{me}/friendCount"), inc(1));
            Some(match db.patch("", &Value::Object(up)) {
                Ok(_) => Ev::RefreshFriends,
                Err(e) => Ev::Toast(e),
            })
        });
        self.say("Wysłano… sprawdzam użytkownika");
    }

    fn save_profile(&mut self) {
        let nick = self.nick_in.trim().to_string();
        let mut up = json!({"avatar": self.avatar_in.trim(), "status": self.status_in.trim()});
        if nick.len() >= 2 {
            up["nick"] = json!(nick.clone());
            up["nickLower"] = json!(nick.to_lowercase());
        }
        let (db, uid) = (self.db.clone(), self.uid.clone());
        let lvl = level_from_xp(n(&self.user, "xp"));
        self.bg(move || {
            let r = db.patch(&format!("users/{uid}"), &up);
            let _ = db.patch(
                &format!("presence/{uid}"),
                &json!({"nick": up["nick"], "avatar": up["avatar"], "level": lvl}),
            );
            Some(match r {
                Ok(_) => Ev::RefreshUser,
                Err(e) => Ev::Toast(e),
            })
        });
        self.say("Profil zapisany!");
    }

    fn go_offline(&mut self) {
        if self.logged && !self.offline_sent {
            self.offline_sent = true;
            let _ = self.db.patch(
                &format!("presence/{}", self.uid),
                &json!({"online": false, "lastSeen": server_ts()}),
            );
        }
    }

    fn logout(&mut self) {
        self.go_offline();
        self.logged = false;
        self.wipe_saved = true;
        *self.db.session.lock().unwrap() = Session::default();
        self.user = Value::Null;
        self.chats.clear();
        self.friends.clear();
        self.my_rx.clear();
        self.presence = Value::Null;
        self.settings_loaded = false;
        self.daily_checked = false;
        self.prev_level = 0;
        self.chat_inflight = false;
        self.hist = vec![View::Chat];
        self.hist_i = 0;
        self.a_pass.clear();
        self.a_pass2.clear();
        self.nav_at = Instant::now();
    }

    fn submit_auth(&mut self) {
        self.a_err.clear();
        self.a_info.clear();
        let email = self.a_email.trim().to_string();
        if self.auth_tab == 0 {
            if email.is_empty() || self.a_pass.is_empty() {
                self.a_err = "Uzupełnij wszystkie pola.".into();
                return;
            }
            let pass = self.a_pass.clone();
            self.a_busy = true;
            self.bg(move || {
                Some(Ev::Auth(firebase::sign_in(&email, &pass).map(|mut s| {
                    s.email = email.clone();
                    s
                })))
            });
        } else {
            let nick = self.a_nick.trim().to_string();
            if nick.is_empty() || email.is_empty() || self.a_pass.is_empty() || self.a_pass2.is_empty() {
                self.a_err = "Uzupełnij wszystkie pola.".into();
            } else if nick.chars().count() < 2 {
                self.a_err = "Nick musi mieć min. 2 znaki.".into();
            } else if self.a_pass != self.a_pass2 {
                self.a_err = "Hasła nie są takie same.".into();
            } else if self.a_pass.len() < 6 {
                self.a_err = "Hasło musi mieć min. 6 znaków.".into();
            } else {
                let pass = self.a_pass.clone();
                self.a_busy = true;
                self.bg(move || Some(Ev::Registered(firebase::register(&nick, &email, &pass))));
            }
        }
    }

    /* ====================== UI ====================== */

    fn titlebar(&mut self, ctx: &Context) {
        let p = self.p;
        let (icon, title) = self.title();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let can_back = self.logged && self.hist_i > 0;
        let can_fwd = self.logged && self.hist_i + 1 < self.hist.len();
        let t = (self.nav_at.elapsed().as_secs_f32() / 0.22).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        if t < 1.0 {
            ctx.request_repaint();
        }
        let online = self
            .presence
            .as_object()
            .map(|o| {
                o.iter()
                    .filter(|(k, v)| k.as_str() != self.uid && v["online"] == true)
                    .count()
            })
            .unwrap_or(0);
        let coins = n(&self.user, "coins");
        let lvl = level_from_xp(n(&self.user, "xp"));
        let logged = self.logged;
        let (mut go_back, mut go_fwd, mut toggle_online) = (false, false, false);

        TopBottomPanel::top("titlebar")
            .exact_height(36.0)
            .frame(Frame::none().fill(p.bar))
            .show(ctx, |ui| {
                let bar = ui.max_rect();
                let drag = ui.interact(bar, Id::new("tb_drag"), Sense::click_and_drag());
                if drag.drag_started_by(PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }
                ui.painter()
                    .line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0_f32, p.border));

                // tytuł w pigułce na środku (zmienia się z zakładką, z krótką animacją)
                let g_icon = ui.painter().layout_no_wrap(icon, FontId::proportional(15.0), p.accent.gamma_multiply(ease));
                let g_title = ui.painter().layout_no_wrap(title, FontId::proportional(14.0), p.text.gamma_multiply(ease));
                let (isz, tsz) = (g_icon.size(), g_title.size());
                let w = isz.x + 8.0 + tsz.x + 30.0;
                let pill = Rect::from_center_size(
                    pos2(bar.center().x + (1.0 - ease) * 10.0, bar.center().y),
                    vec2(w, 24.0),
                );
                ui.painter().rect_filled(pill, 12.0, tint(p.accent, (28.0 * ease) as u8));
                ui.painter().rect_stroke(pill, 12.0, Stroke::new(1.0_f32, tint(p.accent, (60.0 * ease) as u8)));
                let start = pill.left_center() + vec2(15.0, 0.0);
                ui.painter().galley(pos2(start.x, start.y - isz.y / 2.0), g_icon, p.accent);
                ui.painter()
                    .galley(pos2(start.x + isz.x + 8.0, start.y - tsz.y / 2.0), g_title, p.text);

                // lewa strona: wstecz / dalej
                if logged {
                    let r = Rect::from_min_size(bar.min + vec2(6.0, 0.0), vec2(72.0, 36.0));
                    ui.allocate_new_ui(UiBuilder::new().max_rect(r), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if win_icon(ui, WinIcon::Back, 34.0, can_back, &p).clicked() {
                                go_back = true;
                            }
                            if win_icon(ui, WinIcon::Fwd, 34.0, can_fwd, &p).clicked() {
                                go_fwd = true;
                            }
                        });
                    });
                }

                // prawa strona: monety, poziom, online, przyciski okna
                let rr = Rect::from_min_max(pos2(bar.min.x + 120.0, bar.min.y), bar.max);
                ui.allocate_new_ui(UiBuilder::new().max_rect(rr), |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if win_icon(ui, WinIcon::Close, 46.0, true, &p).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        let mx = if maximized { WinIcon::Restore } else { WinIcon::Max };
                        if win_icon(ui, mx, 46.0, true, &p).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                        }
                        if win_icon(ui, WinIcon::Min, 46.0, true, &p).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        if logged {
                            ui.add_space(8.0);
                            if ui
                                .add(
                                    Button::new(RichText::new(format!("👥 {online}")).size(13.0).color(p.text2))
                                        .frame(false),
                                )
                                .on_hover_text("Aktywni teraz")
                                .clicked()
                            {
                                toggle_online = true;
                            }
                            ui.add_space(6.0);
                            Frame::none()
                                .fill(tint(p.gold, 30))
                                .rounding(10.0)
                                .inner_margin(Margin::symmetric(8.0, 1.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("{coins} 🪙")).size(12.0).strong().color(p.gold));
                                });
                            ui.add_space(6.0);
                            Frame::none()
                                .fill(tint(p.accent, 34))
                                .rounding(10.0)
                                .inner_margin(Margin::symmetric(8.0, 1.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("⚡ Lv {lvl}")).size(12.0).strong().color(p.accent));
                                });
                        }
                    });
                });
            });

        if go_back {
            self.back();
        }
        if go_fwd {
            self.forward();
        }
        if toggle_online {
            self.show_online = !self.show_online;
        }
    }

    fn sidebar(&mut self, ctx: &Context) {
        let p = self.p;
        let cur = self.view().clone();
        let mut go: Option<View> = None;
        let mut open_me = false;
        let nick = s(&self.user, "nick");
        let url = s(&self.user, "avatar");
        let ring = frame_color(&s(&self.user, "activeFrame"), ctx.input(|i| i.time));
        let sub = format!(
            "Lv {} · {} 🪙",
            level_from_xp(n(&self.user, "xp")),
            n(&self.user, "coins")
        );
        SidePanel::left("side")
            .exact_width(224.0)
            .resizable(false)
            .frame(Frame::none().fill(p.side).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚡").size(26.0).color(p.accent));
                    ui.vertical(|ui| {
                        ui.add_space(2.0);
                        ui.label(RichText::new("Thunder Chat").size(16.0).strong().color(p.text));
                        ui.label(RichText::new("BŁYSKAWICZNIE SZYBKI").size(9.0).color(p.text3));
                    });
                });
                ui.add_space(16.0);
                let items = [
                    (View::Chat, "💬", "Czat"),
                    (View::Friends, "👥", "Znajomi"),
                    (View::Shop, "🛒", "Sklep"),
                    (View::Settings, "⚙", "Ustawienia"),
                ];
                for (v, icon, label) in items {
                    let active = cur == v || (v == View::Friends && matches!(cur, View::Dm(_)));
                    if nav_row(ui, icon, label, active, &p).clicked() {
                        go = Some(v);
                    }
                    ui.add_space(2.0);
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    let r = Frame::none()
                        .fill(p.panel)
                        .rounding(12.0)
                        .stroke(Stroke::new(1.0_f32, p.border))
                        .inner_margin(Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                avatar(ui, &url, &nick, 32.0, ring);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&nick).strong().color(p.text));
                                    ui.label(RichText::new(&sub).size(11.0).color(p.text3));
                                });
                            });
                        });
                    if r.response.interact(Sense::click()).clicked() {
                        open_me = true;
                    }
                });
            });
        if let Some(v) = go {
            self.go(v);
        }
        if open_me {
            let u = self.uid.clone();
            self.open_profile(u);
        }
    }

    fn input_ui(&mut self, ctx: &Context) {
        let p = self.p;
        let mut send = false;
        let (mut cancel_reply, mut cancel_edit) = (false, false);
        let reply_txt = self.reply.as_ref().map(|r| format!("{}: {}", r.nick, r.text));
        let editing = self.edit.is_some();
        TopBottomPanel::bottom("input")
            .frame(Frame::none().fill(p.bg).inner_margin(Margin::symmetric(16.0, 10.0)))
            .show(ctx, |ui| {
                for (txt, is_edit) in [
                    (reply_txt.clone().map(|t| format!("↩ Odpowiadasz — {t}")), false),
                    (if editing { Some("✏ Edytujesz wiadomość".to_string()) } else { None }, true),
                ] {
                    if let Some(t) = txt {
                        Frame::none()
                            .fill(tint(p.accent, 22))
                            .rounding(10.0)
                            .inner_margin(Margin::symmetric(10.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(t).size(12.0).color(p.text2));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.small_button("✕").clicked() {
                                            if is_edit {
                                                cancel_edit = true;
                                            } else {
                                                cancel_reply = true;
                                            }
                                        }
                                    });
                                });
                            });
                        ui.add_space(6.0);
                    }
                }
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 60.0;
                    let te = ui.add(
                        TextEdit::singleline(&mut self.input)
                            .hint_text("Napisz wiadomość…")
                            .desired_width(w)
                            .char_limit(500)
                            .margin(vec2(12.0, 9.0)),
                    );
                    let enter = te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    let click = ui
                        .add_sized(
                            [46.0, 34.0],
                            Button::new(RichText::new("➤").color(Color32::BLACK)).fill(p.accent).rounding(10.0),
                        )
                        .clicked();
                    if enter || click {
                        send = true;
                        te.request_focus();
                    }
                });
            });
        if cancel_reply {
            self.reply = None;
        }
        if cancel_edit {
            self.edit = None;
            self.input.clear();
        }
        if send {
            self.send();
        }
    }

    fn chat_ui(&mut self, ui: &mut Ui) {
        let Some(path) = self.current_path() else { return };
        let p = self.p;
        let msgs = self.chats.get(&path).map(|c| c.msgs.clone()).unwrap_or_default();
        let me = self.uid.clone();
        let t_now = ui.input(|i| i.time);
        let mut acts: Vec<Act> = vec![];

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if msgs.is_empty() {
                    ui.add_space(80.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("⚡").size(40.0).color(p.accent));
                        ui.label(RichText::new("Tu jest cicho…").size(16.0).strong());
                        ui.label(RichText::new("Napisz pierwszą wiadomość i zacznij rozmowę!").color(p.text3));
                    });
                }
                let mut last_day = String::new();
                for (i, m) in msgs.iter().enumerate() {
                    let day = fmt_day(m.time);
                    if day != last_day {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.vertical_centered(|ui| ui.label(RichText::new(&day).size(11.0).color(p.text3)));
                        last_day = day;
                    }
                    let grouped = i > 0
                        && msgs[i - 1].uid == m.uid
                        && m.time >= msgs[i - 1].time
                        && m.time - msgs[i - 1].time < 240_000
                        && m.reply.is_none()
                        && fmt_day(msgs[i - 1].time) == fmt_day(m.time);
                    ui.add_space(if grouped { 1.0 } else { 10.0 });
                    ui.horizontal_top(|ui| {
                        if grouped {
                            ui.add_space(32.0);
                        } else if avatar(ui, &m.avatar, &m.nick, 32.0, frame_color(&m.frame, t_now)).clicked() {
                            acts.push(Act::Profile(m.uid.clone()));
                        }
                        ui.vertical(|ui| {
                            if !grouped {
                                ui.horizontal(|ui| {
                                    let c = nick_color(ui, m.gold, m.rainbow, &m.name_fx, &p);
                                    ui.label(RichText::new(&m.nick).strong().color(c));
                                    if m.legend {
                                        ui.label(RichText::new("LEGEND").size(9.0).strong().color(Color32::from_rgb(255, 150, 90)));
                                    }
                                    if m.vip {
                                        ui.label(RichText::new("VIP").size(9.0).strong().color(p.accent));
                                    }
                                    if m.level >= 2 {
                                        ui.label(RichText::new(format!("Lv {}", m.level)).size(10.0).color(p.text3));
                                    }
                                    ui.label(RichText::new(fmt_time(m.time)).size(10.0).color(p.text3));
                                });
                            }
                            if let Some((rn, rt)) = &m.reply {
                                ui.label(RichText::new(format!("↩ {rn}: {rt}")).size(11.0).italics().color(p.text3));
                            }
                            ui.label(RichText::new(&m.text).color(if m.media { p.text2 } else { p.text }));
                            if m.edited {
                                ui.label(RichText::new("(edytowano)").size(10.0).italics().color(p.text3));
                            }
                            ui.horizontal(|ui| {
                                for (ty, emo, cnt) in [
                                    ("heart", "❤", m.heart),
                                    ("funny", "😂", m.funny),
                                    ("fire", "🔥", m.fire),
                                ] {
                                    let used = self.my_rx.contains(&format!("{path}|{}|{ty}", m.id));
                                    let b = Button::new(
                                        RichText::new(format!("{emo} {cnt}"))
                                            .size(11.0)
                                            .color(if used { p.accent } else { p.text2 }),
                                    )
                                    .rounding(10.0)
                                    .fill(if used { tint(p.accent, 36) } else { p.input })
                                    .small();
                                    if ui.add(b).clicked() && !used {
                                        acts.push(Act::React(m.id.clone(), m.uid.clone(), ty));
                                    }
                                }
                                ui.menu_button("⋯", |ui| {
                                    if ui.button("↩ Odpowiedz").clicked() {
                                        acts.push(Act::Reply(m.clone()));
                                        ui.close_menu();
                                    }
                                    if !m.media && ui.button("📋 Kopiuj").clicked() {
                                        acts.push(Act::Copy(m.text.clone()));
                                        ui.close_menu();
                                    }
                                    if m.uid == me && !m.media && ui.button("✏ Edytuj").clicked() {
                                        acts.push(Act::Edit(m.clone()));
                                        ui.close_menu();
                                    }
                                    if m.uid != me && ui.button("👤 Profil").clicked() {
                                        acts.push(Act::Profile(m.uid.clone()));
                                        ui.close_menu();
                                    }
                                    if m.uid == me && ui.button("🗑 Usuń").clicked() {
                                        acts.push(Act::Delete(m.id.clone()));
                                        ui.close_menu();
                                    }
                                });
                            });
                        });
                    });
                }
            });
        for a in acts {
            self.apply(a, &path);
        }
    }

    fn friends_ui(&mut self, ui: &mut Ui) {
        let p = self.p;
        let me = self.uid.clone();
        let mut open_dm: Option<String> = None;
        ui.horizontal(|ui| {
            ui.heading("Znajomi");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(Button::new(RichText::new("➕ Dodaj").color(Color32::BLACK)).fill(p.accent).rounding(10.0))
                    .clicked()
                {
                    self.add_friend_open = true;
                }
            });
        });
        ui.label(RichText::new(format!("{} znajomych", self.friends.len())).color(p.text3));
        ui.add_space(8.0);
        let mut order: Vec<&Friend> = self.friends.iter().collect();
        order.sort_by(|a, b| n(&b.last, "at").cmp(&n(&a.last, "at")));
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if order.is_empty() {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("🧑‍🤝‍🧑").size(36.0));
                    ui.label(RichText::new("Nie masz jeszcze znajomych").strong());
                    ui.label(RichText::new("Kliknij „Dodaj” i wpisz dokładny nick.").color(p.text3));
                });
            }
            for f in order {
                let nick = s(&f.user, "nick");
                let mut preview = "Zacznij rozmowę…".to_string();
                if !f.last.is_null() {
                    let pre = if s(&f.last, "fromUid") == me { "Ty: " } else { "" };
                    preview = format!("{pre}{}", s(&f.last, "text"));
                }
                let online = self.presence[&f.uid]["online"] == true;
                let r = Frame::none()
                    .fill(p.panel)
                    .rounding(14.0)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            avatar(ui, &s(&f.user, "avatar"), &nick, 40.0, frame_color(&s(&f.user, "activeFrame"), 0.0));
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&nick).strong().color(nick_color(
                                        ui,
                                        s(&f.user, "activeNameFx") == "gold_nick",
                                        s(&f.user, "activeNameFx") == "rainbow_nick",
                                        &s(&f.user, "activeNameFx"),
                                        &p,
                                    )));
                                    if online {
                                        ui.label(RichText::new("●").size(10.0).color(p.good));
                                    }
                                });
                                ui.label(RichText::new(snippet(&preview)).size(12.0).color(p.text3));
                            });
                        });
                    });
                if r.response.interact(Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    open_dm = Some(f.uid.clone());
                }
                ui.add_space(4.0);
            }
        });
        if let Some(u) = open_dm {
            self.go(View::Dm(u));
        }
    }

    fn shop_ui(&mut self, ui: &mut Ui) {
        let p = self.p;
        let mut act: Option<ShopAct> = None;
        ui.heading("Sklep Monet");
        ui.label(RichText::new("Wydawaj monety zdobyte na czacie").color(p.text3));
        let (cb, xb) = (n(&self.user["inventory"], "boostUntil"), n(&self.user["inventory"], "xpBoostUntil"));
        let now = now_ms();
        if cb > now || xb > now {
            let mut parts = vec![];
            if cb > now {
                parts.push(format!("2x monety ({} min)", (cb - now) / 60_000 + 1));
            }
            if xb > now {
                parts.push(format!("2x XP ({} min)", (xb - now) / 60_000 + 1));
            }
            ui.label(RichText::new(format!("⚡ Aktywne boosty: {}", parts.join(" · "))).color(p.gold));
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for (i, (icon, name)) in CATS.iter().enumerate() {
                if ui.selectable_label(self.shop_cat == i, format!("{icon} {name}")).clicked() {
                    self.shop_cat = i;
                }
            }
        });
        ui.add_space(6.0);
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (idx, item) in ITEMS.iter().enumerate().filter(|(_, it)| it.cat == self.shop_cat) {
                card(ui, &p, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(item.icon).size(26.0));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(item.name).strong());
                            ui.label(RichText::new(item.desc).size(11.0).color(p.text3));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let buy_btn = |t: String| {
                                Button::new(RichText::new(t).strong().color(Color32::BLACK)).fill(p.accent).rounding(10.0)
                            };
                            match item.kind {
                                Kind::Toggle => {
                                    if owns(&self.user, item.key) {
                                        ui.label(RichText::new("Posiadasz ✓").color(p.good));
                                    } else if ui.add(buy_btn(format!("{} 🪙", item.price))).clicked() {
                                        act = Some(ShopAct::Buy(idx));
                                    }
                                }
                                Kind::Equip(field, value) => {
                                    if !owns(&self.user, item.key) {
                                        if ui.add(buy_btn(format!("{} 🪙", item.price))).clicked() {
                                            act = Some(ShopAct::Buy(idx));
                                        }
                                    } else if s(&self.user, field) == value {
                                        if ui.button("Założone ✓").clicked() {
                                            act = Some(ShopAct::Unequip(idx));
                                        }
                                    } else if ui.button("Załóż").clicked() {
                                        act = Some(ShopAct::Equip(idx));
                                    }
                                }
                                Kind::Boost(field) => {
                                    let active = n(&self.user["inventory"], field) > now_ms();
                                    let label = if active { "+1h".to_string() } else { format!("{} 🪙", item.price) };
                                    if ui.add(buy_btn(label)).clicked() {
                                        act = Some(ShopAct::Buy(idx));
                                    }
                                }
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
        });
        match act {
            Some(ShopAct::Buy(i)) => self.buy = Some(i),
            Some(ShopAct::Equip(i)) => {
                if let Kind::Equip(f, v) = ITEMS[i].kind {
                    self.set_field(f, v);
                }
            }
            Some(ShopAct::Unequip(i)) => {
                if let Kind::Equip(f, _) = ITEMS[i].kind {
                    self.set_field(f, "");
                }
            }
            None => {}
        }
    }

    fn settings_ui(&mut self, ui: &mut Ui) {
        let p = self.p;
        let (mut save, mut logout, mut mode) = (false, false, None::<bool>);
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(640.0);
            ui.heading("Ustawienia");
            ui.add_space(8.0);
            card(ui, &p, |ui| {
                ui.label(RichText::new("TWÓJ PROFIL").size(11.0).color(p.text3));
                ui.add_space(6.0);
                ui.label("Nick");
                ui.add(TextEdit::singleline(&mut self.nick_in).desired_width(f32::INFINITY).char_limit(40));
                ui.label("Avatar (link do obrazka)");
                ui.add(TextEdit::singleline(&mut self.avatar_in).desired_width(f32::INFINITY).hint_text("https://…"));
                ui.label("Status");
                ui.add(TextEdit::singleline(&mut self.status_in).desired_width(f32::INFINITY).char_limit(40));
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        Button::new(RichText::new("Zapisz zmiany").strong().color(Color32::BLACK)).fill(p.accent).rounding(10.0),
                    )
                    .clicked()
                {
                    save = true;
                }
            });
            ui.add_space(10.0);
            card(ui, &p, |ui| {
                ui.label(RichText::new("WYGLĄD").size(11.0).color(p.text3));
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.dark, "🌙 Ciemny").clicked() {
                        mode = Some(true);
                    }
                    if ui.selectable_label(!self.dark, "☀ Jasny").clicked() {
                        mode = Some(false);
                    }
                });
            });
            ui.add_space(10.0);
            card(ui, &p, |ui| {
                ui.label(RichText::new("KONTO").size(11.0).color(p.text3));
                ui.label(RichText::new(&self.email).color(p.text2));
                ui.add_space(6.0);
                if ui
                    .add(Button::new(RichText::new("🚪 Wyloguj się").color(p.danger)).rounding(10.0))
                    .clicked()
                {
                    logout = true;
                }
            });
        });
        if save {
            self.save_profile();
        }
        if let Some(d) = mode {
            self.dark = d;
            self.p = if d { Palette::dark() } else { Palette::light() };
            apply_theme(&self.ctx, &self.p, d);
        }
        if logout {
            self.logout();
        }
    }

    fn login_ui(&mut self, ctx: &Context) {
        let p = self.p;
        let mut submit = false;
        CentralPanel::default().frame(Frame::none().fill(p.bg)).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(((ui.available_height() - 480.0) / 2.0).max(12.0));
                Frame::none()
                    .fill(p.panel)
                    .rounding(24.0)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .inner_margin(Margin::same(28.0))
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        ui.label(RichText::new("⚡").size(42.0).color(p.accent));
                        ui.label(RichText::new("Thunder Chat").size(26.0).strong());
                        ui.label(RichText::new("Komunikator bez reklam").size(12.0).color(p.text3));
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.auth_tab == 0, "Logowanie").clicked() {
                                self.auth_tab = 0;
                                self.a_err.clear();
                            }
                            if ui.selectable_label(self.auth_tab == 1, "Rejestracja").clicked() {
                                self.auth_tab = 1;
                                self.a_err.clear();
                            }
                        });
                        ui.add_space(8.0);
                        if self.auth_tab == 1 {
                            ui.add(TextEdit::singleline(&mut self.a_nick).hint_text("Nick").desired_width(340.0).margin(vec2(10.0, 8.0)));
                        }
                        ui.add(TextEdit::singleline(&mut self.a_email).hint_text("Adres e-mail").desired_width(340.0).margin(vec2(10.0, 8.0)));
                        let pw = ui.add(
                            TextEdit::singleline(&mut self.a_pass)
                                .password(true)
                                .hint_text("Hasło")
                                .desired_width(340.0)
                                .margin(vec2(10.0, 8.0)),
                        );
                        let mut enter = pw.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if self.auth_tab == 1 {
                            let pw2 = ui.add(
                                TextEdit::singleline(&mut self.a_pass2)
                                    .password(true)
                                    .hint_text("Powtórz hasło")
                                    .desired_width(340.0)
                                    .margin(vec2(10.0, 8.0)),
                            );
                            enter = pw2.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        }
                        if !self.a_err.is_empty() {
                            ui.colored_label(p.danger, &self.a_err);
                        }
                        if !self.a_info.is_empty() {
                            ui.colored_label(p.good, &self.a_info);
                        }
                        ui.add_space(6.0);
                        let label = if self.a_busy {
                            "Proszę czekać…"
                        } else if self.auth_tab == 0 {
                            "Zaloguj się →"
                        } else {
                            "Utwórz konto →"
                        };
                        let btn = ui.add_enabled(
                            !self.a_busy,
                            Button::new(RichText::new(label).strong().color(Color32::BLACK))
                                .fill(p.accent)
                                .rounding(12.0)
                                .min_size(vec2(340.0, 38.0)),
                        );
                        if self.a_busy {
                            ui.add(Spinner::new());
                        }
                        if (btn.clicked() || enter) && !self.a_busy {
                            submit = true;
                        }
                    });
            });
        });
        if submit {
            self.submit_auth();
        }
    }

    fn windows(&mut self, ctx: &Context) {
        let p = self.p;

        // dzienna seria
        if let Some((streak, coins)) = self.daily {
            let mut claim = false;
            Window::new("🔥 Dzienna seria")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("🔥").size(44.0));
                        ui.label(RichText::new(format!("Dzień {streak}")).size(26.0).strong().color(Color32::from_rgb(255, 157, 59)));
                        ui.label(RichText::new("Wracaj codziennie po większe nagrody!").color(p.text3));
                        ui.add_space(8.0);
                        ui.label(RichText::new(format!("+{coins} 🪙    +25 XP")).strong().color(p.gold));
                        ui.add_space(10.0);
                        if ui
                            .add(Button::new(RichText::new("Odbierz nagrodę ⚡").strong().color(Color32::BLACK)).fill(p.accent).rounding(10.0))
                            .clicked()
                        {
                            claim = true;
                        }
                    });
                });
            if claim {
                self.claim_daily(streak, coins);
            }
        }

        // potwierdzenie zakupu
        if let Some(i) = self.buy {
            let item = &ITEMS[i];
            let (mut yes, mut no) = (false, false);
            Window::new("Potwierdź zakup")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Kupić „{}” za {} 🪙?", item.name, item.price));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(Button::new(RichText::new("Kup").strong().color(Color32::BLACK)).fill(p.accent)).clicked() {
                            yes = true;
                        }
                        if ui.button("Anuluj").clicked() {
                            no = true;
                        }
                    });
                });
            if yes {
                self.buy = None;
                self.do_buy(i);
            } else if no {
                self.buy = None;
            }
        }

        // profil
        if let Some((uid, v)) = self.profile.clone() {
            let mut open = true;
            let (mut dm, mut add) = (false, false);
            let is_me = uid == self.uid;
            let is_friend = self.friends.iter().any(|f| f.uid == uid);
            Window::new("Profil")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    let nick = s(&v, "nick");
                    ui.horizontal(|ui| {
                        avatar(ui, &s(&v, "avatar"), &nick, 56.0, frame_color(&s(&v, "activeFrame"), ctx.input(|i| i.time)));
                        ui.vertical(|ui| {
                            let c = nick_color(ui, false, false, &s(&v, "activeNameFx"), &p);
                            ui.label(RichText::new(&nick).size(18.0).strong().color(c));
                            let mut sub = vec![];
                            if owns(&v, "legend") {
                                sub.push("👑 Legenda".to_string());
                            }
                            if owns(&v, "vip") {
                                sub.push("💎 VIP".to_string());
                            }
                            if n(&v, "streak") > 1 {
                                sub.push(format!("🔥 {}", n(&v, "streak")));
                            }
                            if !s(&v, "status").is_empty() {
                                sub.push(s(&v, "status"));
                            }
                            ui.label(RichText::new(if sub.is_empty() { "Zwykły użytkownik".into() } else { sub.join(" · ") }).color(p.text3));
                        });
                    });
                    ui.add_space(8.0);
                    let (lvl, cur, need) = xp_progress(n(&v, "xp"));
                    ui.horizontal(|ui| {
                        for (val, lab) in [
                            (lvl.to_string(), "Poziom"),
                            (n(&v, "coins").to_string(), "Monety"),
                            (n(&v, "msgCount").to_string(), "Wiadomości"),
                            (n(&v, "heartsReceived").to_string(), "Serduszka"),
                        ] {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(val).strong().color(p.text));
                                ui.label(RichText::new(lab).size(10.0).color(p.text3));
                            });
                            ui.add_space(10.0);
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("{cur} / {need} XP")).size(11.0).color(p.text3));
                    ui.add(ProgressBar::new(cur as f32 / need as f32).desired_width(300.0));
                    ui.add_space(8.0);
                    if !is_me {
                        if is_friend {
                            if ui.button("💬 Napisz wiadomość").clicked() {
                                dm = true;
                            }
                        } else if ui.button("➕ Dodaj do znajomych").clicked() {
                            add = true;
                        }
                    }
                });
            if !open {
                self.profile = None;
            }
            if dm {
                self.profile = None;
                self.go(View::Dm(uid.clone()));
            }
            if add {
                self.profile = None;
                self.add_friend(s(&v, "nick"));
            }
        }

        // aktywni
        if self.show_online {
            let mut open = true;
            let mut pick: Option<String> = None;
            let list: Vec<(String, Value)> = self
                .presence
                .as_object()
                .map(|o| o.iter().filter(|(_, v)| v["online"] == true).map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            Window::new("🟢 Aktywni teraz")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::RIGHT_TOP, [-12.0, 48.0])
                .show(ctx, |ui| {
                    ui.set_min_width(240.0);
                    if list.is_empty() {
                        ui.label(RichText::new("Nikogo nie ma online").color(p.text3));
                    }
                    for (uid, v) in &list {
                        ui.horizontal(|ui| {
                            avatar(ui, &s(v, "avatar"), &s(v, "nick"), 26.0, None);
                            if ui.link(s(v, "nick")).clicked() {
                                pick = Some(uid.clone());
                            }
                            ui.label(RichText::new(format!("Lv {}", n(v, "level"))).size(10.0).color(p.text3));
                        });
                    }
                });
            self.show_online = open;
            if let Some(u) = pick {
                self.open_profile(u);
            }
        }

        // dodawanie znajomego
        if self.add_friend_open {
            let mut open = true;
            let mut go = false;
            Window::new("Dodaj znajomego")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    let r = ui.add(
                        TextEdit::singleline(&mut self.add_friend_in)
                            .hint_text("Wpisz dokładny nick…")
                            .desired_width(280.0)
                            .margin(vec2(10.0, 8.0)),
                    );
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if ui.add(Button::new(RichText::new("Szukaj i dodaj").strong().color(Color32::BLACK)).fill(p.accent)).clicked() || enter {
                        go = true;
                    }
                });
            self.add_friend_open = open;
            if go {
                let nick = std::mem::take(&mut self.add_friend_in);
                self.add_friend_open = false;
                self.add_friend(nick);
            }
        }
    }

    fn draw_toast(&mut self, ctx: &Context) {
        let Some((txt, at)) = self.toast.clone() else { return };
        if at.elapsed() > Duration::from_secs(3) {
            self.toast = None;
            return;
        }
        let p = self.p;
        Area::new(Id::new("toast"))
            .anchor(Align2::CENTER_TOP, [0.0, 48.0])
            .order(Order::Tooltip)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(p.panel)
                    .rounding(12.0)
                    .stroke(Stroke::new(1.0_f32, p.accent))
                    .inner_margin(Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(txt).color(p.text));
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(300));
    }
}

fn poll_chat(db: &Db, path: &str, after: Option<String>) -> Res<Vec<ChatMsg>> {
    let v = match &after {
        None => db.get(path, &[("orderBy", "\"$key\""), ("limitToLast", "50")])?,
        Some(k) => {
            let q = format!("\"{k}\"");
            db.get(path, &[("orderBy", "\"$key\""), ("startAt", q.as_str())])?
        }
    };
    Ok(parse_map(&v))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        while let Ok(ev) = self.rx.try_recv() {
            self.handle(ev);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.go_offline();
        }
        self.tick();
        self.titlebar(ctx);

        if !self.logged {
            self.login_ui(ctx);
            self.draw_toast(ctx);
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        }

        self.sidebar(ctx);
        if matches!(self.view(), View::Chat | View::Dm(_)) {
            self.input_ui(ctx);
        }
        let p = self.p;
        CentralPanel::default()
            .frame(Frame::none().fill(p.bg).inner_margin(Margin::symmetric(16.0, 8.0)))
            .show(ctx, |ui| match self.view().clone() {
                View::Chat | View::Dm(_) => self.chat_ui(ui),
                View::Friends => self.friends_ui(ui),
                View::Shop => self.shop_ui(ui),
                View::Settings => self.settings_ui(ui),
            });
        self.windows(ctx);
        self.draw_toast(ctx);
        ctx.request_repaint_after(Duration::from_millis(400));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark", if self.dark { "1" } else { "0" }.into());
        storage.set_string("email", self.email.clone());
        let rt = if self.logged && !self.wipe_saved {
            self.db.session.lock().map(|s| s.refresh_token.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        storage.set_string("rt", rt);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Thunder Chat")
            .with_decorations(false)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([760.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Thunder Chat",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
