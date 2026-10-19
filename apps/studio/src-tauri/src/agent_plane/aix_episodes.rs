//! Warm edge boxes for AIX views.
//!
//! A live view may call an edge package agent of its own package. The first
//! call starts one `a3s-box` MicroVM for the pair (view token, agent id). Later
//! calls reuse it, keep one session id, and read events after the last
//! sequence. One turn runs at a time. A closed, expired, upgraded, or
//! signed-out view ends the box, and so does an idle limit or a failed turn.
//!
//! The box gets no credential. A package asks for a chat completion or a
//! System One decision with a `model_request` event. The host checks the body,
//! calls the loopback gateway with the member bearer, and posts the answer to
//! `/v1/agent/model-responses`.
//!
//! A turn may carry up to two camera images. The box gets only their names,
//! types, and sizes. A chat body names an image as `aix-image:<name>`, and the
//! host puts the bytes in. Any other image URL is refused.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::edge_box::{RunningBox, start_package_box};
use super::edge_runtime::{
    EdgeResources, PACKAGE_TIMED_OUT, io_failure, post_json_within, start_body,
};
use crate::capabilities::ViewAgent;

/// The box outlives no view session, so its own timeout is the view lifetime.
const EPISODE_LIMIT: Duration = Duration::from_secs(30 * 60);
const START_LIMIT: Duration = Duration::from_secs(90);
const TURN_LIMIT: Duration = Duration::from_secs(90);
const IDLE_LIMIT: Duration = Duration::from_secs(5 * 60);
const MODEL_LIMIT: Duration = Duration::from_secs(60);
const POLL_LIMIT: Duration = Duration::from_secs(10);
const MAX_EPISODES: usize = 2;
const MAX_PROMPT: usize = 16 * 1024;
const MAX_PARAMETERS: usize = 64 * 1024;
const MAX_TEXT: usize = 64 * 1024;
const MAX_PAGE_EVENTS: usize = 256;
const MAX_MODEL_REQUESTS: usize = 16;
const MAX_MODEL_BODY: usize = 256 * 1024;
const MAX_MODEL_RESPONSE: usize = 1024 * 1024;
const MAX_MODEL_MESSAGES: usize = 256;
const MODEL_FIELDS: [&str; 14] = [
    "model",
    "messages",
    "stream",
    "temperature",
    "top_p",
    "max_tokens",
    "max_completion_tokens",
    "stop",
    "seed",
    "response_format",
    "reasoning_effort",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
];
/// The platform's closed set, as `ReasoningEffort` in the API's model service.
const REASONING_EFFORTS: [&str; 9] = [
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra", "auto",
];
const DECISION_FIELDS: [&str; 3] = ["model", "state", "questions"];
const MAX_DECISION_QUESTIONS: usize = 16;
const MAX_TURN_IMAGES: usize = 2;
/// Two encoded images, the prompt, and the parameters fit one authority request body.
const MAX_TURN_IMAGE: usize = 256 * 1024;
const MAX_IMAGE_NAME: usize = 32;
const IMAGE_REFERENCE: &str = "aix-image:";
const IMAGE_TYPES: [(&str, &[u8]); 2] = [
    ("image/jpeg", &[0xff, 0xd8, 0xff]),
    (
        "image/png",
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
    ),
];

/// The gateway surface a `model_request` names. No `api` field means chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelApi {
    ChatCompletions,
    SystemOne,
}

impl ModelApi {
    fn parse(value: Option<&Value>) -> Result<Self, &'static str> {
        match value.map(Value::as_str) {
            None | Some(Some("chat")) => Ok(Self::ChatCompletions),
            Some(Some("systemone")) => Ok(Self::SystemOne),
            Some(_) => Err("unsupported_api: expected chat or systemone"),
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::ChatCompletions => "/v1/chat/completions",
            Self::SystemOne => "/v1/systemone",
        }
    }

    fn body(self, body: Option<&Value>, images: &[TurnImage]) -> Result<Vec<u8>, &'static str> {
        match self {
            Self::ChatCompletions => model_body(body, images),
            Self::SystemOne => decision_body(body),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewTurnBody {
    prompt: String,
    #[serde(default)]
    parameters: Option<Value>,
    #[serde(default)]
    images: Vec<ViewImage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewImage {
    name: String,
    data: String,
}

/// One call from a view. `parameters` is a JSON object when present.
#[derive(Debug)]
pub(crate) struct ViewTurn {
    prompt: String,
    parameters: Option<Value>,
    images: Vec<TurnImage>,
}

/// A JPEG or PNG camera frame for this turn only. The data URL stays on the host.
pub(crate) struct TurnImage {
    name: String,
    media_type: &'static str,
    bytes: usize,
    url: String,
}

impl std::fmt::Debug for TurnImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnImage")
            .field("name", &self.name)
            .field("media_type", &self.media_type)
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl TurnImage {
    fn parse(image: ViewImage) -> Option<Self> {
        let name_ok = image.name.len() <= MAX_IMAGE_NAME
            && image.name.starts_with(|c: char| c.is_ascii_lowercase())
            && image
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b));
        if !name_ok {
            return None;
        }
        let (media_type, encoded) = image.data.strip_prefix("data:")?.split_once(";base64,")?;
        let (media_type, magic) = IMAGE_TYPES
            .into_iter()
            .find(|(known, _)| *known == media_type)?;
        if encoded.len() > MAX_TURN_IMAGE.div_ceil(3) * 4 {
            return None;
        }
        let decoded = STANDARD.decode(encoded).ok()?;
        if decoded.len() > MAX_TURN_IMAGE || !decoded.starts_with(magic) {
            return None;
        }
        Some(Self {
            name: image.name,
            media_type,
            bytes: decoded.len(),
            url: image.data,
        })
    }

    fn manifest(&self) -> Value {
        json!({ "name": self.name, "media_type": self.media_type, "bytes": self.bytes })
    }
}

impl ViewTurn {
    pub(crate) fn parse(body: &[u8]) -> Option<Self> {
        let body: ViewTurnBody = serde_json::from_slice(body).ok()?;
        if body.prompt.trim().is_empty() || body.prompt.len() > MAX_PROMPT {
            return None;
        }
        if let Some(parameters) = &body.parameters
            && (!parameters.is_object() || parameters.to_string().len() > MAX_PARAMETERS)
        {
            return None;
        }
        if body.images.len() > MAX_TURN_IMAGES {
            return None;
        }
        let mut images: Vec<TurnImage> = Vec::with_capacity(body.images.len());
        for image in body.images {
            let image = TurnImage::parse(image)?;
            if images.iter().any(|other| other.name == image.name) {
                return None;
            }
            images.push(image);
        }
        Some(Self {
            prompt: body.prompt,
            parameters: body.parameters,
            images,
        })
    }
}

/// The member gateway as the host sees it. The bearer never leaves the host.
pub(crate) struct ModelAccess {
    pub origin: String,
    pub bearer: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EpisodeError {
    /// A turn is already running for this view and agent.
    Busy,
    /// The warm box limit is reached.
    Capacity,
    /// The box did not start or the turn failed. The box is gone.
    Failed(String),
}

struct Warm {
    running: RunningBox,
    endpoint: String,
    session_id: String,
    cursor: u64,
}

struct Episode {
    entry: PathBuf,
    release: String,
    ended: AtomicBool,
    last_used: Mutex<Instant>,
    warm: tokio::sync::Mutex<Option<Warm>>,
}

impl Episode {
    fn touch(&self) {
        *self
            .last_used
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last_used
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .elapsed()
    }
}

type EpisodeKey = (String, String);

pub(crate) struct AixEpisodes {
    runtime: tokio::runtime::Runtime,
    episodes: Mutex<HashMap<EpisodeKey, Arc<Episode>>>,
    box_cli: Option<PathBuf>,
    image: Option<String>,
    idle_limit: Duration,
}

impl AixEpisodes {
    pub(crate) fn new() -> io::Result<Self> {
        Self::with_box(None, None, IDLE_LIMIT)
    }

    fn with_box(
        box_cli: Option<PathBuf>,
        image: Option<String>,
        idle_limit: Duration,
    ) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("studio-aix-agent")
            .enable_all()
            .build()?;
        Ok(Self {
            runtime,
            episodes: Mutex::new(HashMap::new()),
            box_cli,
            image,
            idle_limit,
        })
    }

    fn episodes(&self) -> std::sync::MutexGuard<'_, HashMap<EpisodeKey, Arc<Episode>>> {
        self.episodes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn episode(
        &self,
        token: &str,
        agent_id: &str,
        agent: &ViewAgent,
    ) -> Result<Arc<Episode>, EpisodeError> {
        let key = (token.to_owned(), agent_id.to_owned());
        let mut episodes = self.episodes();
        if let Some(current) = episodes.get(&key) {
            if current.entry == agent.entry && current.release == agent.release_identity {
                return Ok(Arc::clone(current));
            }
            if let Some(stale) = episodes.remove(&key) {
                self.end(stale);
            }
        }
        if episodes.len() >= MAX_EPISODES {
            return Err(EpisodeError::Capacity);
        }
        let episode = Arc::new(Episode {
            entry: agent.entry.clone(),
            release: agent.release_identity.clone(),
            ended: AtomicBool::new(false),
            last_used: Mutex::new(Instant::now()),
            warm: tokio::sync::Mutex::new(None),
        });
        episodes.insert(key, Arc::clone(&episode));
        Ok(episode)
    }

    fn forget(&self, token: &str, agent_id: &str, episode: &Arc<Episode>) {
        let key = (token.to_owned(), agent_id.to_owned());
        let mut episodes = self.episodes();
        if episodes
            .get(&key)
            .is_some_and(|current| Arc::ptr_eq(current, episode))
        {
            episodes.remove(&key);
        }
    }

    /// Run one turn in the warm box for `token` and `agent_id`. The caller has
    /// already bound `agent` to the live view.
    pub(crate) fn turn(
        &self,
        token: &str,
        agent_id: &str,
        agent: &ViewAgent,
        turn: &ViewTurn,
        model: &ModelAccess,
    ) -> Result<String, EpisodeError> {
        let episode = self.episode(token, agent_id, agent)?;
        let Ok(mut warm) = episode.warm.try_lock() else {
            return Err(EpisodeError::Busy);
        };
        if episode.ended.load(Ordering::SeqCst) {
            return Err(EpisodeError::Failed("the view closed".to_owned()));
        }
        episode.touch();
        let outcome = self.runtime.block_on(async {
            if warm.is_none() {
                *warm = Some(self.start(agent).await?);
            }
            let Some(live) = warm.as_mut() else {
                return Err("the edge box did not start".to_owned());
            };
            let deadline = Instant::now() + TURN_LIMIT;
            run_turn(
                live,
                &episode.ended,
                &agent.release_identity,
                turn,
                model,
                deadline,
            )
            .await
        });
        episode.touch();
        if outcome.is_ok() && !episode.ended.load(Ordering::SeqCst) {
            return outcome.map_err(EpisodeError::Failed);
        }
        let taken = warm.take();
        drop(warm);
        self.forget(token, agent_id, &episode);
        let stderr = match taken {
            Some(live) => self.runtime.block_on(live.running.finish()),
            None => String::new(),
        };
        match outcome {
            Ok(_) => Err(EpisodeError::Failed("the view closed".to_owned())),
            Err(error) if stderr.trim().is_empty() => Err(EpisodeError::Failed(error)),
            Err(error) => Err(EpisodeError::Failed(format!("{error}\n{}", stderr.trim()))),
        }
    }

    async fn start(&self, agent: &ViewAgent) -> Result<Warm, String> {
        match agent
            .entry
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("mjs" | "js" | "cjs") => {}
            _ => return Err("package entry is not a node program".to_owned()),
        }
        let resources = EdgeResources {
            cpu_millis: agent.cpu_millis,
            memory_bytes: agent.memory_bytes,
        };
        let start = start_package_box(
            &agent.entry,
            &agent.declared_entry,
            &resources,
            EPISODE_LIMIT,
            self.box_cli.as_deref(),
            self.image.as_deref(),
        );
        let running = tokio::time::timeout(START_LIMIT, start)
            .await
            .map_err(|_| PACKAGE_TIMED_OUT.to_owned())??;
        Ok(Warm {
            endpoint: format!("http://127.0.0.1:{}", running.host_port),
            running,
            session_id: uuid::Uuid::now_v7().to_string(),
            cursor: 0,
        })
    }

    /// End the box now, or when its running turn returns.
    fn end(&self, episode: Arc<Episode>) {
        episode.ended.store(true, Ordering::SeqCst);
        let taken = episode
            .warm
            .try_lock()
            .ok()
            .and_then(|mut warm| warm.take());
        if let Some(live) = taken {
            self.runtime.spawn(async move {
                live.running.finish().await;
            });
        }
    }

    /// Every box of a view the renderer closed.
    pub(crate) fn end_view(&self, token: &str) {
        let ended: Vec<_> = {
            let mut episodes = self.episodes();
            let keys: Vec<_> = episodes
                .keys()
                .filter(|(view, _)| view == token)
                .cloned()
                .collect();
            keys.iter().filter_map(|key| episodes.remove(key)).collect()
        };
        for episode in ended {
            self.end(episode);
        }
    }

    /// End idle boxes and boxes whose view no longer binds the agent.
    pub(crate) fn reap(&self, alive: impl Fn(&str, &str) -> bool) {
        let entries: Vec<_> = self
            .episodes()
            .iter()
            .map(|(key, episode)| (key.clone(), Arc::clone(episode)))
            .collect();
        for ((token, agent), episode) in entries {
            if episode.idle_for() < self.idle_limit && alive(&token, &agent) {
                continue;
            }
            self.forget(&token, &agent, &episode);
            self.end(episode);
        }
    }

    pub(crate) fn has_episodes(&self) -> bool {
        !self.episodes().is_empty()
    }

    /// Remove every box before the process exits.
    pub(crate) fn shutdown(&self) {
        let ended: Vec<_> = self
            .episodes()
            .drain()
            .map(|(_, episode)| episode)
            .collect();
        let mut finishing = Vec::new();
        for episode in ended {
            episode.ended.store(true, Ordering::SeqCst);
            if let Some(live) = episode
                .warm
                .try_lock()
                .ok()
                .and_then(|mut warm| warm.take())
            {
                finishing.push(self.runtime.spawn(live.running.finish()));
            }
        }
        self.runtime.block_on(async {
            let _ = tokio::time::timeout(Duration::from_secs(25), async {
                for task in finishing {
                    let _ = task.await;
                }
            })
            .await;
        });
    }
}

async fn run_turn(
    warm: &mut Warm,
    ended: &AtomicBool,
    release: &str,
    turn: &ViewTurn,
    model: &ModelAccess,
    deadline: Instant,
) -> Result<String, String> {
    let run_id = uuid::Uuid::now_v7().to_string();
    let mut command = start_body(&turn.prompt, &warm.session_id, &run_id, release);
    if let (Some(object), Some(parameters)) = (command.as_object_mut(), &turn.parameters) {
        object.insert("parameters".to_owned(), parameters.clone());
    }
    if let Some(object) = command.as_object_mut()
        && !turn.images.is_empty()
    {
        let manifest = turn.images.iter().map(TurnImage::manifest).collect();
        object.insert("images".to_owned(), Value::Array(manifest));
    }
    post_ready(&warm.endpoint, "/v1/agent/commands", &command, deadline).await?;
    let mut text = String::new();
    let mut model_requests = 0;
    let mut pause = Duration::from_millis(20);
    loop {
        if ended.load(Ordering::SeqCst) {
            return Err("the view closed".to_owned());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let page = post_json_within(
            &warm.endpoint,
            "/v1/agent/events:page",
            &json!({ "after": warm.cursor }),
            remaining.min(POLL_LIMIT),
        )
        .await?;
        let events = page
            .get("events")
            .and_then(Value::as_array)
            .ok_or_else(|| "agent runtime returned no events".to_owned())?;
        if events.len() > MAX_PAGE_EVENTS {
            return Err("agent runtime returned too many events".to_owned());
        }
        let mut progressed = false;
        for event in events {
            let sequence = event
                .get("sequence")
                .and_then(Value::as_u64)
                .ok_or_else(|| "agent event has no sequence".to_owned())?;
            if sequence <= warm.cursor {
                continue;
            }
            warm.cursor = sequence;
            progressed = true;
            if event
                .get("run_id")
                .and_then(Value::as_str)
                .is_some_and(|other| other != run_id)
            {
                continue;
            }
            match event.get("type").and_then(Value::as_str) {
                Some("text_delta") => {
                    text.push_str(event.get("text").and_then(Value::as_str).unwrap_or(""));
                }
                Some("output") => {
                    text = event
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                }
                Some("failed") => {
                    return Err(event
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("agent runtime failed")
                        .to_owned());
                }
                Some("completed") => {
                    if text.is_empty() {
                        return Err("agent runtime returned no text".to_owned());
                    }
                    return Ok(text);
                }
                Some("model_request") => {
                    model_requests += 1;
                    if model_requests > MAX_MODEL_REQUESTS {
                        return Err("agent exceeded the model request limit".to_owned());
                    }
                    answer_model_request(&warm.endpoint, event, &turn.images, model, deadline)
                        .await?;
                }
                _ => {}
            }
            if text.len() > MAX_TEXT {
                return Err("agent output exceeds the limit".to_owned());
            }
        }
        if progressed {
            pause = Duration::from_millis(20);
            continue;
        }
        let slice = pause.min(deadline.saturating_duration_since(Instant::now()));
        if slice.is_zero() {
            return Err(PACKAGE_TIMED_OUT.to_owned());
        }
        tokio::time::sleep(slice).await;
        pause = (pause * 2).min(Duration::from_millis(100));
    }
}

/// The published port accepts before the guest proxy listens. A reset or a
/// refusal before a status line means the command was not delivered.
async fn post_ready(
    endpoint: &str,
    path: &str,
    body: &Value,
    deadline: Instant,
) -> Result<Value, String> {
    let mut pause = Duration::from_millis(50);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match post_json_within(endpoint, path, body, remaining).await {
            Ok(value) => return Ok(value),
            Err(error) if not_delivered(&error) => {
                let slice = pause.min(deadline.saturating_duration_since(Instant::now()));
                if slice.is_zero() {
                    return Err(error);
                }
                tokio::time::sleep(slice).await;
                pause = (pause * 2).min(Duration::from_millis(250));
            }
            Err(error) => return Err(error),
        }
    }
}

fn not_delivered(error: &str) -> bool {
    [
        "connection reset",
        "connection refused",
        "broken pipe",
        "returned no status",
    ]
    .iter()
    .any(|phrase| error.contains(phrase))
}

async fn answer_model_request(
    endpoint: &str,
    event: &Value,
    images: &[TurnImage],
    model: &ModelAccess,
    deadline: Instant,
) -> Result<(), String> {
    let request = event
        .get("request")
        .and_then(Value::as_str)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
        .ok_or_else(|| "model request has no valid id".to_owned())?;
    let checked = ModelApi::parse(event.get("api"))
        .and_then(|api| Ok((api, api.body(event.get("body"), images)?)));
    let (status, body) = match (checked, model.bearer.as_deref()) {
        (Err(reason), _) => (400, json!({ "error": reason })),
        (Ok(_), None) => (401, json!({ "error": "unauthorized" })),
        (Ok((api, bytes)), Some(bearer)) => {
            let limit = deadline
                .saturating_duration_since(Instant::now())
                .min(MODEL_LIMIT);
            gateway_post(&model.origin, api.path(), bearer, &bytes, limit).await
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    post_json_within(
        endpoint,
        "/v1/agent/model-responses",
        &json!({ "request": request, "status": status, "body": body }),
        remaining,
    )
    .await
    .map(|_| ())
}

/// A non-streaming chat completion with known fields only. A content part is
/// text or a reference to one of this turn's images, each used at most once.
fn model_body(body: Option<&Value>, images: &[TurnImage]) -> Result<Vec<u8>, &'static str> {
    let object = body.and_then(Value::as_object).ok_or("invalid_body")?;
    if object
        .keys()
        .any(|key| !MODEL_FIELDS.contains(&key.as_str()))
    {
        return Err("unsupported_field");
    }
    checked_model(object)?;
    if object.get("reasoning_effort").is_some_and(|effort| {
        !effort
            .as_str()
            .is_some_and(|effort| REASONING_EFFORTS.contains(&effort))
    }) {
        return Err(
            "invalid_reasoning_effort: expected none, minimal, low, medium, high, xhigh, max, ultra or auto",
        );
    }
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or("invalid_messages")?;
    if messages.is_empty() || messages.len() > MAX_MODEL_MESSAGES {
        return Err("invalid_messages");
    }
    if object
        .get("stream")
        .is_some_and(|stream| stream != &Value::Bool(false))
    {
        return Err("streaming_unsupported");
    }
    let mut checked: Map<String, Value> = object.clone();
    checked.insert("stream".to_owned(), Value::Bool(false));
    let sent = serde_json::to_vec(&checked).map_err(|_| "invalid_body")?;
    if sent.len() > MAX_MODEL_BODY {
        return Err("body_too_large");
    }
    let mut used = Vec::with_capacity(images.len());
    let Some(Value::Array(messages)) = checked.get_mut("messages") else {
        return Err("invalid_messages");
    };
    for message in messages {
        if let Some(Value::Array(parts)) = message.get_mut("content") {
            for part in parts {
                attach_image(part, images, &mut used)?;
            }
        }
    }
    if used.is_empty() {
        return Ok(sent);
    }
    serde_json::to_vec(&checked).map_err(|_| "invalid_body")
}

fn attach_image<'a>(
    part: &mut Value,
    images: &'a [TurnImage],
    used: &mut Vec<&'a str>,
) -> Result<(), &'static str> {
    let part = part.as_object_mut().ok_or("unsupported_content")?;
    match part.get("type").and_then(Value::as_str) {
        Some("text") if part.len() == 2 && part.get("text").is_some_and(Value::is_string) => {
            return Ok(());
        }
        Some("image_url") if part.len() == 2 => {}
        _ => return Err("unsupported_content"),
    }
    let image_url = part
        .get_mut("image_url")
        .and_then(Value::as_object_mut)
        .ok_or("unsupported_image")?;
    let detail_ok = image_url
        .get("detail")
        .is_none_or(|detail| matches!(detail.as_str(), Some("auto" | "low" | "high")));
    if !detail_ok || image_url.keys().any(|key| key != "url" && key != "detail") {
        return Err("unsupported_image");
    }
    let image = image_url
        .get("url")
        .and_then(Value::as_str)
        .and_then(|url| url.strip_prefix(IMAGE_REFERENCE))
        .and_then(|name| images.iter().find(|image| image.name == name))
        .ok_or("unsupported_image")?;
    if used.contains(&image.name.as_str()) {
        return Err("image_reused");
    }
    used.push(&image.name);
    image_url.insert("url".to_owned(), Value::String(image.url.clone()));
    Ok(())
}

/// A System One decision request: a named model, a state, and its questions.
fn decision_body(body: Option<&Value>) -> Result<Vec<u8>, &'static str> {
    let object = body.and_then(Value::as_object).ok_or("invalid_body")?;
    if object
        .keys()
        .any(|key| !DECISION_FIELDS.contains(&key.as_str()))
    {
        return Err("unsupported_field");
    }
    checked_model(object)?;
    if !object
        .get("state")
        .is_some_and(|state| state.is_object() || state.is_array() || state.is_string())
    {
        return Err("invalid_state");
    }
    let questions = object
        .get("questions")
        .and_then(Value::as_object)
        .ok_or("invalid_questions")?;
    if questions.is_empty()
        || questions.len() > MAX_DECISION_QUESTIONS
        || !questions.values().all(Value::is_object)
    {
        return Err("invalid_questions");
    }
    let bytes = serde_json::to_vec(object).map_err(|_| "invalid_body")?;
    if bytes.len() > MAX_MODEL_BODY {
        return Err("body_too_large");
    }
    Ok(bytes)
}

fn checked_model(object: &Map<String, Value>) -> Result<(), &'static str> {
    let model = object.get("model").and_then(Value::as_str).unwrap_or("");
    if model.is_empty() || model.len() > 128 || model.chars().any(char::is_control) {
        return Err("invalid_model");
    }
    Ok(())
}

/// `POST` a checked body to `path` on the loopback gateway. HTTP/1.0 keeps
/// the reply unchunked; a chunked reply is decoded anyway.
async fn gateway_post(
    origin: &str,
    path: &str,
    bearer: &str,
    body: &[u8],
    limit: Duration,
) -> (u16, Value) {
    let unavailable = || (502, json!({ "error": "gateway_unavailable" }));
    let Some(address) = crate::org_login::parse_loopback(origin) else {
        return unavailable();
    };
    if bearer.is_empty()
        || bearer
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return (401, json!({ "error": "unauthorized" }));
    }
    let exchange = async {
        let mut stream = TcpStream::connect(address).await.map_err(io_failure)?;
        let head = format!(
            "POST {path} HTTP/1.0\r\nHost: {address}\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAuthorization: Bearer {bearer}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(head.as_bytes())
            .await
            .map_err(io_failure)?;
        stream.write_all(body).await.map_err(io_failure)?;
        let mut response = Vec::new();
        let cap = (MAX_MODEL_RESPONSE + 16 * 1024) as u64;
        (&mut stream)
            .take(cap + 1)
            .read_to_end(&mut response)
            .await
            .map_err(io_failure)?;
        if response.len() as u64 > cap {
            return Err("response_too_large".to_owned());
        }
        Ok::<_, String>(response)
    };
    let response = match tokio::time::timeout(limit, exchange).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) if error == "response_too_large" => {
            return (502, json!({ "error": "response_too_large" }));
        }
        _ => return unavailable(),
    };
    let Some(split) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return unavailable();
    };
    let head = String::from_utf8_lossy(&response[..split]);
    let Some(status) = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
    else {
        return unavailable();
    };
    let chunked = head.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    });
    let raw = &response[split + 4..];
    let decoded = if chunked {
        dechunk(raw)
    } else {
        Some(raw.to_vec())
    };
    match decoded.and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()) {
        Some(value) if value.to_string().len() <= MAX_MODEL_RESPONSE => (status, value),
        Some(_) => (502, json!({ "error": "response_too_large" })),
        None => (status, json!({ "error": "invalid_response" })),
    }
}

fn dechunk(mut raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = raw.windows(2).position(|window| window == b"\r\n")?;
        let size_text = std::str::from_utf8(&raw[..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        raw = &raw[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        if raw.len() < size + 2 || out.len() + size > MAX_MODEL_RESPONSE {
            return None;
        }
        out.extend_from_slice(&raw[..size]);
        raw = &raw[size + 2..];
    }
}

#[cfg(test)]
#[path = "aix_episodes_tests.rs"]
mod tests;
