//! Firebase przez REST (bez przeglądarki, bez webview).
//! Auth: identitytoolkit / securetoken. Baza: Realtime Database REST API.

use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const API_KEY: &str = "AIzaSyCQXm-oMsOZg80gMrANDwxEbr2jWoV2I8A";
pub const DB_URL: &str = "https://mojczat-d50e5-default-rtdb.europe-west1.firebasedatabase.app";
const AUTH: &str = "https://identitytoolkit.googleapis.com/v1";

pub type Res<T> = Result<T, String>;

#[derive(Clone, Default)]
pub struct Session {
    pub uid: String,
    pub email: String,
    pub id_token: String,
    pub refresh_token: String,
    pub expires_at: Option<Instant>,
}

pub fn server_ts() -> Value {
    json!({".sv": "timestamp"})
}
pub fn inc(n: i64) -> Value {
    json!({".sv": {"increment": n}})
}
pub fn room_id(a: &str, b: &str) -> String {
    let mut v = [a, b];
    v.sort();
    v.join("_")
}

fn tr(code: &str) -> String {
    let c = code.split(':').next().unwrap_or(code).trim();
    let s = match c {
        "EMAIL_NOT_FOUND" | "INVALID_PASSWORD" | "INVALID_LOGIN_CREDENTIALS" => {
            "Nieprawidłowy e-mail lub hasło."
        }
        "EMAIL_EXISTS" => "Ten e-mail jest już zajęty.",
        "INVALID_EMAIL" => "Nieprawidłowy adres e-mail.",
        "TOO_MANY_ATTEMPTS_TRY_LATER" => "Za dużo prób. Spróbuj później.",
        "USER_DISABLED" => "To konto zostało zablokowane.",
        "TOKEN_EXPIRED" | "INVALID_REFRESH_TOKEN" | "USER_NOT_FOUND" => {
            "Sesja wygasła — zaloguj się ponownie."
        }
        s if s.starts_with("WEAK_PASSWORD") => "Hasło jest za słabe (min. 6 znaków).",
        other => return format!("Błąd: {other}"),
    };
    s.to_string()
}

fn map_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, r) => {
            let v: Value = r.into_json().unwrap_or(Value::Null);
            if code == 401 || code == 403 {
                if let Some(m) = v["error"]["message"].as_str() {
                    return tr(m);
                }
                return "Brak uprawnień — sprawdź reguły Firebase.".into();
            }
            if let Some(m) = v["error"]["message"].as_str() {
                return tr(m);
            }
            if let Some(m) = v["error"].as_str() {
                return m.to_string();
            }
            format!("Błąd serwera ({code})")
        }
        other => format!("Brak połączenia z siecią ({other})"),
    }
}

fn auth_post(endpoint: &str, body: Value) -> Res<Value> {
    ureq::post(&format!("{AUTH}/{endpoint}?key={API_KEY}"))
        .timeout(Duration::from_secs(15))
        .send_json(body)
        .map_err(map_err)?
        .into_json::<Value>()
        .map_err(|e| e.to_string())
}

fn session_from(r: &Value) -> Session {
    let secs: u64 = r["expiresIn"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);
    Session {
        uid: r["localId"].as_str().unwrap_or("").to_string(),
        email: r["email"].as_str().unwrap_or("").to_string(),
        id_token: r["idToken"].as_str().unwrap_or("").to_string(),
        refresh_token: r["refreshToken"].as_str().unwrap_or("").to_string(),
        expires_at: Some(Instant::now() + Duration::from_secs(secs)),
    }
}

pub fn sign_in(email: &str, pass: &str) -> Res<Session> {
    let r = auth_post(
        "accounts:signInWithPassword",
        json!({"email": email, "password": pass, "returnSecureToken": true}),
    )?;
    let tok = r["idToken"].as_str().ok_or("Brak tokenu")?.to_string();
    let look = auth_post("accounts:lookup", json!({"idToken": tok}))?;
    if look["users"][0]["emailVerified"].as_bool() != Some(true) {
        return Err("Najpierw kliknij link weryfikacyjny wysłany na Twój e-mail.".into());
    }
    Ok(session_from(&r))
}

pub fn register(nick: &str, email: &str, pass: &str) -> Res<()> {
    let r = auth_post(
        "accounts:signUp",
        json!({"email": email, "password": pass, "returnSecureToken": true}),
    )?;
    let sess = session_from(&r);
    let _ = auth_post(
        "accounts:sendOobCode",
        json!({"requestType": "VERIFY_EMAIL", "idToken": sess.id_token}),
    );
    let db = Db::new(sess.clone());
    db.patch(
        &format!("users/{}", sess.uid),
        &json!({
            "nick": nick, "nickLower": nick.to_lowercase(), "avatar": "", "status": "",
            "coins": 100, "xp": 0, "msgCount": 0, "heartsReceived": 0, "mediaCount": 0,
            "createdAt": server_ts(), "friendCount": 0
        }),
    )?;
    Ok(())
}

pub fn refresh(rt: &str) -> Res<Session> {
    let r: Value = ureq::post(&format!(
        "https://securetoken.googleapis.com/v1/token?key={API_KEY}"
    ))
    .timeout(Duration::from_secs(15))
    .send_form(&[("grant_type", "refresh_token"), ("refresh_token", rt)])
    .map_err(map_err)?
    .into_json()
    .map_err(|e| e.to_string())?;
    let secs: u64 = r["expires_in"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);
    Ok(Session {
        uid: r["user_id"].as_str().unwrap_or("").to_string(),
        email: String::new(),
        id_token: r["id_token"].as_str().unwrap_or("").to_string(),
        refresh_token: r["refresh_token"].as_str().unwrap_or(rt).to_string(),
        expires_at: Some(Instant::now() + Duration::from_secs(secs)),
    })
}

#[derive(Clone)]
pub struct Db {
    pub session: Arc<Mutex<Session>>,
}

impl Db {
    pub fn new(s: Session) -> Self {
        Db { session: Arc::new(Mutex::new(s)) }
    }

    /// Zwraca ważny token (odświeża, gdy zostało < 5 min).
    fn token(&self) -> Res<String> {
        let mut s = self.session.lock().map_err(|_| "lock".to_string())?;
        let stale = s
            .expires_at
            .map(|e| Instant::now() + Duration::from_secs(300) >= e)
            .unwrap_or(false);
        if stale && !s.refresh_token.is_empty() {
            let n = refresh(&s.refresh_token)?;
            s.id_token = n.id_token;
            s.refresh_token = n.refresh_token;
            s.expires_at = n.expires_at;
        }
        Ok(s.id_token.clone())
    }

    pub fn request(
        &self,
        method: &str,
        path: &str,
        q: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Res<Value> {
        let tok = self.token()?;
        let url = format!("{DB_URL}/{}.json", path.trim_matches('/'));
        let mut req = ureq::request(method, &url)
            .timeout(Duration::from_secs(20))
            .query("auth", &tok);
        for (k, v) in q {
            req = req.query(k, v);
        }
        let resp = match body {
            Some(b) => req.send_json(b),
            None => req.call(),
        }
        .map_err(map_err)?;
        resp.into_json::<Value>().map_err(|e| e.to_string())
    }

    pub fn get(&self, path: &str, q: &[(&str, &str)]) -> Res<Value> {
        self.request("GET", path, q, None)
    }
    pub fn patch(&self, path: &str, b: &Value) -> Res<Value> {
        self.request("PATCH", path, &[], Some(b))
    }
    pub fn post(&self, path: &str, b: &Value) -> Res<Value> {
        self.request("POST", path, &[], Some(b))
    }
    pub fn delete(&self, path: &str) -> Res<Value> {
        self.request("DELETE", path, &[], None)
    }
}
