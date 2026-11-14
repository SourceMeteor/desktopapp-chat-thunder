use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
pub fn n(v: &Value, k: &str) -> i64 {
    v.get(k)
        .and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}
pub fn owns(user: &Value, key: &str) -> bool {
    user["inventory"][key] == true
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn level_from_xp(xp: i64) -> i64 {
    let mut l = 1;
    while xp >= 50 * (l + 1) * l {
        l += 1;
    }
    l
}
/// (poziom, xp w poziomie, xp potrzebne na poziom)
pub fn xp_progress(xp: i64) -> (i64, i64, i64) {
    let l = level_from_xp(xp);
    let base = 50 * l * (l - 1);
    (l, (xp - base).max(0), 100 * l)
}

pub fn fmt_time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}
pub fn fmt_day(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.with_timezone(&chrono::Local).format("%d.%m.%Y").to_string())
        .unwrap_or_default()
}
pub fn today_str() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}
pub fn yesterday_str() -> String {
    (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string()
}

const BAD: [&str; 8] = [
    "dupa", "kurde", "brzydkie", "jebać", "kurwa", "skurwysynie", "fuck you", "fuck",
];
pub fn sanitize(t: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let low: Vec<char> = t
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    let mut out = String::new();
    let mut i = 0;
    'outer: while i < chars.len() {
        for w in BAD {
            let wc: Vec<char> = w.chars().collect();
            if low.len() >= i + wc.len() && low[i..i + wc.len()] == wc[..] {
                out.push_str("***");
                i += wc.len();
                continue 'outer;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub fn snippet(t: &str) -> String {
    let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 90 {
        format!("{}…", t.chars().take(89).collect::<String>())
    } else {
        t
    }
}

#[derive(Clone, Default)]
pub struct ChatMsg {
    pub id: String,
    pub uid: String,
    pub nick: String,
    pub avatar: String,
    pub frame: String,
    pub text: String,
    pub time: i64,
    pub level: i64,
    pub vip: bool,
    pub legend: bool,
    pub gold: bool,
    pub rainbow: bool,
    pub name_fx: String,
    pub heart: i64,
    pub funny: i64,
    pub fire: i64,
    pub reply: Option<(String, String)>,
    pub edited: bool,
    pub media: bool,
}

impl ChatMsg {
    pub fn parse(id: &str, v: &Value) -> ChatMsg {
        let kind = s(v, "type");
        let (text, media) = match kind.as_str() {
            "image" => ("📷 Zdjęcie — zobacz w wersji webowej".to_string(), true),
            "video" => ("🎥 Film — zobacz w wersji webowej".to_string(), true),
            "voice" => ("🎙 Wiadomość głosowa — odsłuchasz ją w wersji webowej".to_string(), true),
            _ => (s(v, "text"), false),
        };
        let reply = if v["replyTo"].is_object() {
            Some((s(&v["replyTo"], "nick"), s(&v["replyTo"], "text")))
        } else {
            None
        };
        ChatMsg {
            id: id.to_string(),
            uid: s(v, "uid"),
            nick: s(v, "nick"),
            avatar: s(v, "avatar"),
            frame: s(v, "frame"),
            text,
            time: n(v, "time"),
            level: n(v, "level"),
            vip: v["isVip"] == true,
            legend: v["legend"] == true,
            gold: v["goldNick"] == true,
            rainbow: v["rainbowNick"] == true,
            name_fx: s(v, "nameFx"),
            heart: n(&v["reactions"], "heart"),
            funny: n(&v["reactions"], "funny"),
            fire: n(&v["reactions"], "fire"),
            reply,
            edited: v.get("editedAt").is_some(),
            media,
        }
    }
}

pub fn parse_map(v: &Value) -> Vec<ChatMsg> {
    let mut out: Vec<ChatMsg> = match v.as_object() {
        Some(o) => o
            .iter()
            .filter(|(_, m)| m.is_object())
            .map(|(k, m)| ChatMsg::parse(k, m))
            .collect(),
        None => vec![],
    };
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/* ---------------- SKLEP ---------------- */

pub enum Kind {
    Toggle,
    Equip(&'static str, &'static str), // pole, wartość
    Boost(&'static str),               // pole z czasem końca
}
pub struct Item {
    pub key: &'static str,
    pub cat: usize,
    pub icon: &'static str,
    pub price: i64,
    pub kind: Kind,
    pub name: &'static str,
    pub desc: &'static str,
}

pub const CATS: [(&str, &str); 5] = [
    ("👑", "Rangi"),
    ("🖼", "Ramki"),
    ("✨", "Nicki"),
    ("🌀", "Aury"),
    ("⚡", "Boosty"),
];

const fn it(
    key: &'static str,
    cat: usize,
    icon: &'static str,
    price: i64,
    kind: Kind,
    name: &'static str,
    desc: &'static str,
) -> Item {
    Item { key, cat, icon, price, kind, name, desc }
}

pub const ITEMS: &[Item] = &[
    it("vip", 0, "💎", 5000, Kind::Toggle, "Ranga VIP", "Odznaka VIP pod nickiem i świecące wiadomości"),
    it("legend", 0, "👑", 15000, Kind::Toggle, "Ranga Legenda", "Ekskluzywna złoto-różowa odznaka Legendy"),
    it("frame_neon", 1, "🔷", 1800, Kind::Equip("activeFrame", "frame_neon"), "Neonowa Ramka", "Avatar świeci turkusowym neonem"),
    it("frame_fire", 1, "🔥", 1800, Kind::Equip("activeFrame", "frame_fire"), "Ognista Ramka", "Avatar płonie ognistą poświatą"),
    it("frame_gold", 1, "🥇", 3500, Kind::Equip("activeFrame", "frame_gold"), "Złota Ramka", "Złoty blask wokół avatara"),
    it("frame_ice", 1, "❄", 3500, Kind::Equip("activeFrame", "frame_ice"), "Lodowa Ramka", "Mroźna, lodowa poświata"),
    it("frame_rainbow", 1, "🌈", 5000, Kind::Equip("activeFrame", "frame_rainbow"), "Tęczowa Ramka", "Animowana, przelewająca się kolorami"),
    it("frame_void", 1, "🌌", 6500, Kind::Equip("activeFrame", "frame_void"), "Ramka Pustki", "Fioletowa aura z innego wymiaru"),
    it("frame_thunder", 1, "⚡", 12000, Kind::Equip("activeFrame", "frame_thunder"), "Ramka Grzmotu", "Najmocniejsza ramka — wyładowania elektryczne"),
    it("gold_nick", 2, "✨", 2500, Kind::Equip("activeNameFx", "gold_nick"), "Złoty Nick", "Twój nick świeci złotem"),
    it("rainbow_nick", 2, "🌈", 6000, Kind::Equip("activeNameFx", "rainbow_nick"), "Tęczowy Nick", "Animowany nick przelewający się kolorami"),
    it("frost_nick", 2, "🧊", 4000, Kind::Equip("activeNameFx", "frost_nick"), "Mroźny Nick", "Lodowo-błękitny gradient"),
    it("plasma_nick", 2, "💜", 5500, Kind::Equip("activeNameFx", "plasma_nick"), "Plazmowy Nick", "Gradient róż–fiolet–błękit"),
    it("aura_cyan", 3, "🌀", 2200, Kind::Equip("aura", "aura-cyan"), "Aura Cyjan", "Błękitna poświata profilu"),
    it("aura_gold", 3, "🌟", 3000, Kind::Equip("aura", "aura-gold"), "Złota Aura", "Złota poświata profilu"),
    it("aura_fire", 3, "🔥", 3000, Kind::Equip("aura", "aura-fire"), "Ognista Aura", "Płomienna poświata profilu"),
    it("aura_ice", 3, "🧊", 3000, Kind::Equip("aura", "aura-ice"), "Lodowa Aura", "Mroźna poświata profilu"),
    it("aura_rainbow", 3, "🌈", 5500, Kind::Equip("aura", "aura-rainbow"), "Tęczowa Aura", "Animowana aura profilu"),
    it("aura_void", 3, "🌌", 7000, Kind::Equip("aura", "aura-void"), "Aura Pustki", "Pulsująca fioletowa aura"),
    it("coin_boost", 4, "🪙", 2000, Kind::Boost("boostUntil"), "Boost 2x Monet (1h)", "Podwójne monety za każdą wiadomość"),
    it("xp_boost", 4, "⚡", 2500, Kind::Boost("xpBoostUntil"), "Boost 2x XP (1h)", "Podwójne XP za każdą wiadomość"),
];
