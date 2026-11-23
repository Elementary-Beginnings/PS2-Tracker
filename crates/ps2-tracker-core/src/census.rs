//! Client for the Daybreak Census REST API (read only).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::CharacterId;
use crate::ratelimit::{Backoff, RateLimiter};

pub const CENSUS_BASE: &str = "https://census.daybreakgames.com";

#[derive(Debug, thiserror::Error)]
pub enum CensusError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Census reports: {0}")]
    Api(String),
    #[error("unexpected response: {0}")]
    Shape(String),
    #[error("character not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterInfo {
    #[serde(with = "crate::events::id_string")]
    pub character_id: CharacterId,
    pub name: String,
    pub faction_id: u32,
    pub world_id: Option<u32>,
    pub battle_rank: u32,
    pub prestige_level: u32,
}

/// Lifetime stats of a character (Census `characters_stat_history`,
/// headshots from `characters_weapon_stat_by_faction`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LifetimeStats {
    pub kills: u64,
    pub deaths: u64,
    pub score: u64,
    pub play_time_secs: u64,
    pub facility_captures: u64,
    pub facility_defends: u64,
    pub weapon_kills: u64,
    pub weapon_headshots: u64,
    /// per weapon (item_id): kills, headshots
    pub weapons: BTreeMap<u32, WeaponLifetime>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponLifetime {
    pub kills: u64,
    pub headshots: u64,
}

impl LifetimeStats {
    pub fn kd(&self) -> f64 {
        if self.deaths > 0 {
            self.kills as f64 / self.deaths as f64
        } else {
            self.kills as f64
        }
    }
    pub fn kpm(&self) -> f64 {
        if self.play_time_secs >= 60 {
            self.kills as f64 / (self.play_time_secs as f64 / 60.0)
        } else {
            0.0
        }
    }
    pub fn hsr(&self) -> f64 {
        if self.weapon_kills > 0 {
            self.weapon_headshots as f64 / self.weapon_kills as f64
        } else {
            0.0
        }
    }
    pub fn score_per_hour(&self) -> f64 {
        if self.play_time_secs >= 60 {
            self.score as f64 / (self.play_time_secs as f64 / 3600.0)
        } else {
            0.0
        }
    }
}

#[derive(Clone)]
pub struct CensusClient {
    http: reqwest::Client,
    base: String,
    service_id: String,
    limiter: Arc<RateLimiter>,
    /// Separate slot queue for lookups a user is waiting for (see
    /// [`CensusClient::interactive`]), so they never queue behind background work.
    interactive_limiter: Arc<RateLimiter>,
    retries: u32,
    /// Per-request timeout below the client-wide 45 s (interactive lookups).
    request_timeout: Option<Duration>,
}

/// Without the service ID (logs).
impl std::fmt::Debug for CensusClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CensusClient")
            .field("base", &self.base)
            .field("service_id", &"***")
            .field("retries", &self.retries)
            .finish_non_exhaustive()
    }
}

fn s<T: std::str::FromStr>(v: &Value, key: &str) -> Option<T> {
    match v.get(key)? {
        Value::String(x) => x.parse().ok(),
        Value::Number(n) => n.to_string().parse().ok(),
        _ => None,
    }
}

impl CensusClient {
    pub fn new(service_id: &str) -> Result<Self, CensusError> {
        Self::with_base(CENSUS_BASE, service_id)
    }

    pub fn with_base(base: &str, service_id: &str) -> Result<Self, CensusError> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("ps2-tracker/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(45))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            service_id: service_id.to_owned(),
            // Census has no documented limit; 4 requests/s is conservative.
            limiter: Arc::new(RateLimiter::new(Duration::from_millis(250))),
            interactive_limiter: Arc::new(RateLimiter::new(Duration::from_millis(250))),
            retries: 3,
            request_timeout: None,
        })
    }

    /// The same client for lookups a user is waiting for (adding a character or
    /// squad member, outfit search): its own rate-limit queue, so background name
    /// resolution during play cannot delay it, one retry and a 15 s timeout, so a
    /// slow Census answers with an error instead of blocking the form.
    pub fn interactive(&self) -> Self {
        Self {
            limiter: Arc::clone(&self.interactive_limiter),
            retries: 1,
            request_timeout: Some(Duration::from_secs(15)),
            ..self.clone()
        }
    }

    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    fn url(&self, collection: &str) -> String {
        format!(
            "{}/s:{}/get/ps2:v2/{}",
            self.base, self.service_id, collection
        )
    }

    /// Downloads a static Census image (`/files/ps2/images/static/<id>.png`;
    /// the path carries no service ID). Only PNG, at most `max_bytes`.
    pub async fn static_image(&self, id: u32, max_bytes: usize) -> Result<Vec<u8>, CensusError> {
        self.limiter.acquire().await;
        let url = format!("{}/files/ps2/images/static/{id}.png", self.base);
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| CensusError::Http(e.without_url()))?;
        let ty = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !ty.starts_with("image/png") {
            return Err(CensusError::Shape(format!(
                "image {id}: content type {ty:?}"
            )));
        }
        if resp.content_length().is_some_and(|n| n > max_bytes as u64) {
            return Err(CensusError::Shape(format!("image {id}: too large")));
        }
        let mut resp = resp;
        let mut out = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| CensusError::Http(e.without_url()))?
        {
            out.extend_from_slice(&chunk);
            if out.len() > max_bytes {
                return Err(CensusError::Shape(format!("image {id}: too large")));
            }
        }
        Ok(out)
    }

    /// Fetches a collection and returns the `<collection>_list` array.
    pub async fn get(
        &self,
        collection: &str,
        query: &[(&str, String)],
    ) -> Result<Vec<Value>, CensusError> {
        let mut backoff = Backoff::new(Duration::from_millis(500), Duration::from_secs(8));
        loop {
            self.limiter.acquire().await;
            // reqwest errors carry the request URL, which contains the service
            // ID: strip it before the error reaches a log line or the UI
            let res = self.get_once(collection, query).await.map_err(|e| match e {
                CensusError::Http(h) => CensusError::Http(h.without_url()),
                other => other,
            });
            match res {
                Ok(v) => return Ok(v),
                Err(e) if backoff.attempt() < self.retries && is_transient(&e) => {
                    let d = backoff.next_delay();
                    tracing::debug!(?d, error = %e, collection, "retrying Census request");
                    tokio::time::sleep(d).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn get_once(
        &self,
        collection: &str,
        query: &[(&str, String)],
    ) -> Result<Vec<Value>, CensusError> {
        let mut req = self.http.get(self.url(collection)).query(query);
        if let Some(t) = self.request_timeout {
            req = req.timeout(t);
        }
        let resp = req.send().await?.error_for_status()?;
        let body: Value = resp.json().await?;
        parse_list(collection, body)
    }

    pub async fn character_by_name(&self, name: &str) -> Result<CharacterInfo, CensusError> {
        let rows = self
            .get(
                "character",
                &[
                    ("name.first_lower", name.trim().to_lowercase()),
                    (
                        "c:show",
                        "character_id,name.first,faction_id,battle_rank,prestige_level".into(),
                    ),
                    ("c:resolve", "world".into()),
                ],
            )
            .await?;
        rows.first()
            .and_then(parse_character)
            .ok_or_else(|| CensusError::NotFound(name.to_owned()))
    }

    pub async fn character_by_id(&self, id: CharacterId) -> Result<CharacterInfo, CensusError> {
        let rows = self
            .get(
                "character",
                &[
                    ("character_id", id.to_string()),
                    (
                        "c:show",
                        "character_id,name.first,faction_id,battle_rank,prestige_level".into(),
                    ),
                    ("c:resolve", "world".into()),
                ],
            )
            .await?;
        rows.first()
            .and_then(parse_character)
            .ok_or_else(|| CensusError::NotFound(id.to_string()))
    }

    pub async fn lifetime(&self, id: CharacterId) -> Result<LifetimeStats, CensusError> {
        let hist = self
            .get(
                "characters_stat_history",
                &[
                    ("character_id", id.to_string()),
                    ("c:show", "stat_name,all_time".into()),
                    ("c:limit", "50".into()),
                ],
            )
            .await?;
        let mut out = parse_stat_history(&hist);
        let kills = self.weapon_stat(id, "weapon_kills").await?;
        let hs = self.weapon_stat(id, "weapon_headshots").await?;
        merge_weapon_stats(&mut out, &kills, &hs);
        Ok(out)
    }

    async fn weapon_stat(&self, id: CharacterId, stat: &str) -> Result<Vec<Value>, CensusError> {
        self.get(
            "characters_weapon_stat_by_faction",
            &[
                ("character_id", id.to_string()),
                ("stat_name", stat.into()),
                ("c:show", "item_id,value_vs,value_nc,value_tr".into()),
                ("c:limit", "5000".into()),
            ],
        )
        .await
    }

    /// Weapon/item names (batched, at most 100 ids per request).
    pub async fn item_names(&self, ids: &[u32]) -> Result<HashMap<u32, String>, CensusError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(100) {
            let list = chunk
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let rows = self
                .get(
                    "item",
                    &[
                        ("item_id", list),
                        ("c:show", "item_id,name.en".into()),
                        ("c:limit", chunk.len().to_string()),
                    ],
                )
                .await?;
            for r in rows {
                if let (Some(id), Some(name)) = (s::<u32>(&r, "item_id"), name_en(&r)) {
                    out.insert(id, name);
                }
            }
        }
        Ok(out)
    }

    /// Weapon names with Census category (`item_category_id`), batches of 100.
    pub async fn item_names_with_category(
        &self,
        ids: &[u32],
    ) -> Result<HashMap<u32, (String, Option<u32>)>, CensusError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(100) {
            let list = chunk
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let rows = self
                .get(
                    "item",
                    &[
                        ("item_id", list),
                        ("c:show", "item_id,name.en,item_category_id".into()),
                        ("c:limit", chunk.len().to_string()),
                    ],
                )
                .await?;
            for r in rows {
                if let (Some(id), Some(name)) = (s::<u32>(&r, "item_id"), name_en(&r)) {
                    out.insert(id, (name, s(&r, "item_category_id")));
                }
            }
        }
        Ok(out)
    }

    /// All item categories (e.g. 6 → "Pistol", 11 → "Sniper Rifle").
    pub async fn item_categories(&self) -> Result<HashMap<u32, String>, CensusError> {
        let rows = self
            .get(
                "item_category",
                &[
                    ("c:show", "item_category_id,name.en".into()),
                    ("c:limit", "500".into()),
                ],
            )
            .await?;
        Ok(rows
            .iter()
            .filter_map(|r| Some((s::<u32>(r, "item_category_id")?, name_en(r)?)))
            .collect())
    }

    /// Server names (`world`), only for the given IDs.
    pub async fn world_names(&self, ids: &[u32]) -> Result<HashMap<u32, String>, CensusError> {
        self.names_by_id("world", "world_id", ids).await
    }

    /// Continent names (`zone`), only for the given zone definition IDs.
    pub async fn zone_names(&self, ids: &[u32]) -> Result<HashMap<u32, String>, CensusError> {
        self.names_by_id("zone", "zone_id", ids).await
    }

    async fn names_by_id(
        &self,
        collection: &str,
        key: &str,
        ids: &[u32],
    ) -> Result<HashMap<u32, String>, CensusError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(100) {
            let list = chunk
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let rows = self
                .get(
                    collection,
                    &[
                        (key, list),
                        ("c:show", format!("{key},name.en")),
                        ("c:limit", chunk.len().to_string()),
                    ],
                )
                .await?;
            out.extend(
                rows.iter()
                    .filter_map(|r| Some((s::<u32>(r, key)?, name_en(r)?))),
            );
        }
        Ok(out)
    }

    pub async fn vehicle_names(&self) -> Result<HashMap<u32, String>, CensusError> {
        let rows = self
            .get(
                "vehicle",
                &[
                    ("c:show", "vehicle_id,name.en".into()),
                    ("c:limit", "500".into()),
                ],
            )
            .await?;
        Ok(rows
            .iter()
            .filter_map(|r| Some((s::<u32>(r, "vehicle_id")?, name_en(r)?)))
            .collect())
    }

    /// Members of an outfit, found by tag (alias) or name. Read-only public
    /// data: `outfit` and `outfit_member` with the character's name, faction
    /// and online status joined in. At most `limit` members (largest outfits
    /// have thousands).
    pub async fn outfit_members(
        &self,
        tag_or_name: &str,
        limit: usize,
    ) -> Result<(String, Vec<OutfitMember>), CensusError> {
        let q = tag_or_name.trim().to_lowercase();
        if q.is_empty() || q.len() > 32 {
            return Err(CensusError::NotFound(tag_or_name.to_owned()));
        }
        let mut outfit = None;
        for key in ["alias_lower", "name_lower"] {
            let rows = self
                .get(
                    "outfit",
                    &[(key, q.clone()), ("c:show", "outfit_id,name,alias".into())],
                )
                .await?;
            if let Some(r) = rows.first() {
                outfit = Some((
                    s::<u64>(r, "outfit_id").unwrap_or(0),
                    r.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                ));
                break;
            }
        }
        let Some((oid, oname)) = outfit.filter(|(id, _)| *id != 0) else {
            return Err(CensusError::NotFound(tag_or_name.to_owned()));
        };
        let rows = self
            .get(
                "outfit_member",
                &[
                    ("outfit_id", oid.to_string()),
                    ("c:limit", limit.min(5000).to_string()),
                    ("c:show", "character_id".into()),
                    (
                        "c:join",
                        "character^show:name.first'faction_id^inject_at:character".into(),
                    ),
                ],
            )
            .await?;
        let mut members = parse_outfit_members(&rows);
        // characters_online_status cannot be joined (checked against the live
        // API), so it is asked for separately, 100 IDs per request
        let ids: Vec<CharacterId> = members.iter().map(|m| m.character_id).collect();
        let mut online = std::collections::HashSet::new();
        for chunk in ids.chunks(100) {
            let list = chunk
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let rows = self
                .get(
                    "characters_online_status",
                    &[("character_id", list), ("c:limit", chunk.len().to_string())],
                )
                .await?;
            for r in rows {
                let on = r
                    .get("online_status")
                    .and_then(Value::as_str)
                    .is_some_and(|x| x != "0");
                if let (true, Some(id)) = (on, s::<u64>(&r, "character_id")) {
                    online.insert(id);
                }
            }
        }
        for m in &mut members {
            m.online = online.contains(&m.character_id);
        }
        members.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok((oname, members))
    }

    pub async fn character_names(
        &self,
        ids: &[CharacterId],
    ) -> Result<HashMap<CharacterId, (String, u32)>, CensusError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(100) {
            let list = chunk
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let rows = self
                .get(
                    "character",
                    &[
                        ("character_id", list),
                        ("c:show", "character_id,name.first,faction_id".into()),
                        ("c:limit", chunk.len().to_string()),
                    ],
                )
                .await?;
            for r in rows {
                let id = s::<u64>(&r, "character_id");
                let name = r.pointer("/name/first").and_then(Value::as_str);
                if let (Some(id), Some(name)) = (id, name) {
                    out.insert(id, (name.to_owned(), s(&r, "faction_id").unwrap_or(0)));
                }
            }
        }
        Ok(out)
    }
}

/// One member of an outfit (for the squad import).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutfitMember {
    #[serde(with = "crate::events::id_string")]
    pub character_id: CharacterId,
    pub name: String,
    pub faction_id: Option<u32>,
    pub online: bool,
}

fn parse_outfit_members(rows: &[Value]) -> Vec<OutfitMember> {
    let mut v: Vec<OutfitMember> = rows
        .iter()
        .filter_map(|r| {
            let id = s::<u64>(r, "character_id")?;
            let c = r.get("character")?;
            let name = c.pointer("/name/first")?.as_str()?.to_owned();
            Some(OutfitMember {
                character_id: id,
                name,
                faction_id: s(c, "faction_id"),
                online: false,
            })
        })
        .collect();
    v.sort_by_key(|m| m.name.to_lowercase());
    v
}

fn is_transient(e: &CensusError) -> bool {
    match e {
        CensusError::Http(h) => {
            h.is_timeout() || h.is_connect() || h.status().is_some_and(|s| s.is_server_error())
        }
        CensusError::Api(m) => m.contains("SERVER_ERROR") || m.contains("timeout"),
        _ => false,
    }
}

fn name_en(v: &Value) -> Option<String> {
    v.pointer("/name/en")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Checks a Census response for errors and returns `<collection>_list`.
pub fn parse_list(collection: &str, body: Value) -> Result<Vec<Value>, CensusError> {
    if let Some(e) = body.get("error") {
        return Err(CensusError::Api(e.as_str().unwrap_or("unknown").to_owned()));
    }
    if let Some(code) = body.get("errorCode") {
        let msg = body
            .get("errorMessage")
            .and_then(Value::as_str)
            .unwrap_or("");
        return Err(CensusError::Api(format!(
            "{} {msg}",
            code.as_str().unwrap_or("?")
        )));
    }
    let key = format!("{collection}_list");
    match body.get(&key) {
        Some(Value::Array(a)) => Ok(a.clone()),
        _ => Err(CensusError::Shape(format!("`{key}` missing"))),
    }
}

pub fn parse_character(v: &Value) -> Option<CharacterInfo> {
    Some(CharacterInfo {
        character_id: s(v, "character_id")?,
        name: v.pointer("/name/first")?.as_str()?.to_owned(),
        faction_id: s(v, "faction_id").unwrap_or(0),
        world_id: s(v, "world_id"),
        battle_rank: v
            .pointer("/battle_rank/value")
            .and_then(Value::as_str)
            .and_then(|x| x.parse().ok())
            .unwrap_or(0),
        prestige_level: s(v, "prestige_level").unwrap_or(0),
    })
}

pub fn parse_stat_history(rows: &[Value]) -> LifetimeStats {
    let mut out = LifetimeStats::default();
    for r in rows {
        let Some(name) = r.get("stat_name").and_then(Value::as_str) else {
            continue;
        };
        let v: u64 = s(r, "all_time").unwrap_or(0);
        match name {
            "kills" => out.kills = v,
            "deaths" => out.deaths = v,
            "score" => out.score = v,
            "time" => out.play_time_secs = v,
            "facility_capture" => out.facility_captures = v,
            "facility_defend" => out.facility_defends = v,
            _ => {}
        }
    }
    out
}

pub fn merge_weapon_stats(out: &mut LifetimeStats, kills: &[Value], headshots: &[Value]) {
    let sum = |r: &Value| -> u64 {
        ["value_vs", "value_nc", "value_tr"]
            .iter()
            .map(|k| s::<u64>(r, k).unwrap_or(0))
            .sum()
    };
    for r in kills {
        if let Some(id) = s::<u32>(r, "item_id") {
            let k = sum(r);
            out.weapons.entry(id).or_default().kills += k;
            out.weapon_kills += k;
        }
    }
    for r in headshots {
        if let Some(id) = s::<u32>(r, "item_id") {
            let h = sum(r);
            out.weapons.entry(id).or_default().headshots += h;
            out.weapon_headshots += h;
        }
    }
}

/// Name cache so Census is not queried for every event.
#[derive(Debug, Default)]
pub struct NameCache {
    pub items: HashMap<u32, String>,
    pub vehicles: HashMap<u32, String>,
    pub characters: HashMap<CharacterId, (String, u32)>,
    /// Server names (`world_id` → name).
    pub worlds: HashMap<u32, String>,
    /// Continent names (zone definition ID → name).
    pub zones: HashMap<u32, String>,
    /// Server/continent IDs Census was already asked for in this run
    /// (`(false, id)` = server, `(true, id)` = continent); not asked again.
    pub asked_locations: std::collections::HashSet<(bool, u32)>,
    /// Characters Census returned nothing for (deleted, or an NPC/environment
    /// ID), with the time of the last attempt. Retried after [`NOT_FOUND_RETRY`].
    pub not_found: HashMap<CharacterId, i64>,
}

/// Seconds before a character Census did not know is asked for again.
pub const NOT_FOUND_RETRY: i64 = 600;

pub type SharedNames = Arc<Mutex<NameCache>>;

impl NameCache {
    /// Copy of the server and continent names (small maps), so callers do not
    /// hold the name cache lock while taking other locks.
    pub fn clone_locations(&self) -> (HashMap<u32, String>, HashMap<u32, String>) {
        (self.worlds.clone(), self.zones.clone())
    }

    /// Server and continent IDs without a name that were not asked for yet.
    pub fn locations_to_fetch(
        &self,
        ids: impl IntoIterator<Item = (Option<u32>, Option<u32>)>,
    ) -> (Vec<u32>, Vec<u32>) {
        let (mut w, mut z) = (Vec::new(), Vec::new());
        for (world, zone) in ids {
            if let Some(i) = world
                && !self.worlds.contains_key(&i)
                && !self.asked_locations.contains(&(false, i))
            {
                w.push(i);
            }
            if let Some(i) = zone
                && !self.zones.contains_key(&i)
                && !self.asked_locations.contains(&(true, i))
            {
                z.push(i);
            }
        }
        for v in [&mut w, &mut z] {
            v.sort_unstable();
            v.dedup();
        }
        (w, z)
    }

    pub fn missing_items(&self, ids: impl IntoIterator<Item = u32>) -> Vec<u32> {
        let mut v: Vec<u32> = ids
            .into_iter()
            .filter(|i| *i != 0 && !self.items.contains_key(i))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
    pub fn missing_characters(
        &self,
        ids: impl IntoIterator<Item = CharacterId>,
    ) -> Vec<CharacterId> {
        let mut v: Vec<CharacterId> = ids
            .into_iter()
            .filter(|i| *i != 0 && !self.characters.contains_key(i))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
    /// Like [`missing_characters`](Self::missing_characters), but leaves out
    /// characters Census did not know less than [`NOT_FOUND_RETRY`] seconds ago.
    pub fn characters_to_fetch(
        &self,
        ids: impl IntoIterator<Item = CharacterId>,
        now: i64,
    ) -> Vec<CharacterId> {
        let mut v = self.missing_characters(ids);
        v.retain(|i| {
            self.not_found
                .get(i)
                .is_none_or(|t| now - t >= NOT_FOUND_RETRY)
        });
        v
    }
    /// Records the outcome of a lookup: requested IDs without an answer are
    /// marked as not found, found ones are cached.
    pub fn record_characters(
        &mut self,
        requested: &[CharacterId],
        found: HashMap<CharacterId, (String, u32)>,
        now: i64,
    ) {
        for id in requested {
            if found.contains_key(id) {
                self.not_found.remove(id);
            } else {
                self.not_found.insert(*id, now);
            }
        }
        self.characters.extend(found);
    }
}

#[cfg(test)]
mod tests {

    /// The service ID must not appear in error messages (logs, UI).
    #[tokio::test]
    async fn errors_do_not_contain_the_service_id() {
        let c = CensusClient::with_base("http://127.0.0.1:9", "secretid123").unwrap();
        let err = c.get("character", &[]).await.unwrap_err().to_string();
        assert!(!err.contains("secretid123"), "{err}");
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn parses_error_and_list() {
        let e = parse_list(
            "character",
            json!({"error":"Provided Service ID is not registered."}),
        );
        assert!(matches!(e, Err(CensusError::Api(_))));
        let e = parse_list(
            "character",
            json!({"errorCode":"SERVER_ERROR","errorMessage":"x"}),
        );
        assert!(matches!(e, Err(CensusError::Api(m)) if m.contains("SERVER_ERROR")));
        let ok = parse_list(
            "character",
            json!({"character_list":[{"a":1}],"returned":1}),
        )
        .unwrap();
        assert_eq!(ok.len(), 1);
        assert!(matches!(
            parse_list("item", json!({"returned":0})),
            Err(CensusError::Shape(_))
        ));
    }

    #[test]
    fn parses_character() {
        let v = json!({"character_id":"5428000000000000042","name":{"first":"SampleSoldier","first_lower":"samplesoldier"},
            "faction_id":"3","battle_rank":{"percent_to_next":"0","value":"100"},"prestige_level":"2","world_id":"10"});
        let c = parse_character(&v).unwrap();
        assert_eq!(c.character_id, 5428000000000000042);
        assert_eq!(c.faction_id, 3);
        assert_eq!(c.battle_rank, 100);
        assert_eq!(c.prestige_level, 2);
        assert_eq!(c.world_id, Some(10));
    }

    #[test]
    fn parses_lifetime() {
        let hist = vec![
            json!({"stat_name":"kills","all_time":"1000"}),
            json!({"stat_name":"deaths","all_time":"500"}),
            json!({"stat_name":"time","all_time":"36000"}),
            json!({"stat_name":"score","all_time":"720000"}),
        ];
        let mut l = parse_stat_history(&hist);
        let kills = vec![json!({"item_id":"80","value_vs":"300","value_nc":"200","value_tr":"0"})];
        let hs = vec![json!({"item_id":"80","value_vs":"100","value_nc":"50","value_tr":"0"})];
        merge_weapon_stats(&mut l, &kills, &hs);
        assert!((l.kd() - 2.0).abs() < 1e-9);
        assert!((l.kpm() - 1000.0 / 600.0).abs() < 1e-9);
        assert!((l.hsr() - 0.3).abs() < 1e-9);
        assert!((l.score_per_hour() - 72000.0).abs() < 1e-9);
        assert_eq!(
            l.weapons[&80],
            WeaponLifetime {
                kills: 500,
                headshots: 150
            }
        );
    }

    #[test]
    fn parses_outfit_members() {
        let rows = vec![
            json!({"character_id":"5428000000000000002","character":{"name":{"first":"Bravo"},"faction_id":"3"}}),
            json!({"character_id":"5428000000000000001","character":{"name":{"first":"alpha"},"faction_id":"3"}}),
            json!({"character_id":"5428000000000000003"}),
        ];
        let m = parse_outfit_members(&rows);
        assert_eq!(m.len(), 2, "member without joined character skipped");
        assert_eq!(m[0].name, "alpha");
        assert_eq!(m[1].faction_id, Some(3));
    }

    #[test]
    fn missing_ids_are_deduped() {
        let mut c = NameCache::default();
        c.record_characters(&[5, 6], HashMap::from([(5, ("A".into(), 1))]), 100);
        assert_eq!(c.characters_to_fetch([5, 6, 7], 200), vec![7]);
        assert_eq!(c.characters_to_fetch([6], 100 + NOT_FOUND_RETRY), vec![6]);
        let mut c = NameCache::default();
        c.items.insert(80, "Gauss".into());
        assert_eq!(c.missing_items([80, 0, 7, 7, 3]), vec![3, 7]);
    }
}
