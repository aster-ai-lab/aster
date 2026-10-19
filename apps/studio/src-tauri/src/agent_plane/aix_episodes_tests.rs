#![cfg(test)]

use super::*;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;

const BEARER: &str = "member-secret-token";

/// A stateful package behind a fake `a3s-box run`. Each command asks the host
/// for one completion, then answers with the turn number, its session, the
/// model status, and the model text. A command with images asks about each
/// image by reference.
const FAKE_BOX: &str = r#"#!/usr/bin/env node
import http from 'node:http'
import { appendFileSync } from 'node:fs'
const args = process.argv.slice(2)
const log = (name, text) => appendFileSync(new URL(name, import.meta.url), text + '\n')
log('./args.log', args.join('\0'))
log('./env.log', JSON.stringify(process.env))
if (args[0] === 'image-inspect' || args[0] === 'rm') process.exit(0)
const published = args[args.indexOf('-p') + 1].split(':')[0]
const events = []
let sequence = 0
let turns = 0
let current
const push = (event) => events.push({ sequence: ++sequence, run_id: current.run, ...event })
const server = http.createServer((request, response) => {
  const chunks = []
  request.on('data', (chunk) => chunks.push(chunk))
  request.on('end', () => {
    const body = JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}')
    response.setHeader('content-type', 'application/json')
    if (request.url === '/v1/agent/commands') {
      turns += 1
      current = { run: body.identity.run_id, session: body.identity.session_id, turn: turns }
      log('./commands.log', JSON.stringify(body))
      const content = body.images
        ? [{ type: 'text', text: body.prompt }, ...body.images.map((image) => ({ type: 'image_url', image_url: { url: `aix-image:${image.name}` } }))]
        : body.prompt
      if (body.prompt === 'fail') push({ type: 'failed', message: 'the package refused' })
      else push({ type: 'model_request', request: `r${turns}`, body: { model: 'gw-model', messages: [{ role: 'user', content }] } })
      response.end(JSON.stringify({ accepted: true }))
      return
    }
    if (request.url === '/v1/agent/model-responses') {
      const text = body.body?.choices?.[0]?.message?.content ?? body.body?.error ?? ''
      push({ type: 'output', text: `${current.turn}:${current.session}:${body.status}:${text}` })
      push({ type: 'completed' })
      response.end(JSON.stringify({ accepted: true }))
      return
    }
    response.end(JSON.stringify({ events: events.filter((event) => event.sequence > (Number(body.after) || 0)) }))
  })
})
server.listen(Number(published), '127.0.0.1', () => process.stdout.write('Box fake_box (5f7eafd3b0e3) started.\n'))
"#;

struct Lab {
    dir: PathBuf,
    agent: ViewAgent,
    episodes: AixEpisodes,
}

impl Lab {
    fn new(name: &str) -> Option<Self> {
        if std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_err()
        {
            return None;
        }
        let dir =
            std::env::temp_dir().join(format!("a3s-aix-episode-{name}-{}", uuid::Uuid::now_v7()));
        let entry = dir.join("package").join("agent").join("run.mjs");
        std::fs::create_dir_all(entry.parent()?).ok()?;
        std::fs::write(&entry, "// served by the fake box").ok()?;
        let cli = dir.join("a3s-box.mjs");
        std::fs::write(&cli, FAKE_BOX).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).ok()?;
        }
        let episodes = AixEpisodes::with_box(Some(cli), None, IDLE_LIMIT).ok()?;
        Some(Self {
            agent: agent_at(entry),
            dir,
            episodes,
        })
    }

    fn log(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }

    fn runs(&self) -> usize {
        self.log("args.log")
            .lines()
            .filter(|line| line.starts_with("run\0"))
            .count()
    }

    fn removed(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.log("args.log").contains("rm\0-f\0fake_box") {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

impl Drop for Lab {
    fn drop(&mut self) {
        self.episodes.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn agent_at(entry: PathBuf) -> ViewAgent {
    ViewAgent {
        entry,
        declared_entry: "agent/run.mjs".to_owned(),
        release_identity: format!("sha256:{}", "a".repeat(64)),
        cpu_millis: 500,
        memory_bytes: 268_435_456,
    }
}

fn prompt(text: &str) -> ViewTurn {
    ViewTurn::parse(json!({ "prompt": text }).to_string().as_bytes()).unwrap()
}

/// A loopback gateway that answers every completion with `hold` and reports
/// each request's authorization header and body.
fn fake_gateway() -> (String, mpsc::Receiver<(String, Value)>) {
    fake_gateway_replying("hold")
}

fn fake_gateway_replying(content: &'static str) -> (String, mpsc::Receiver<(String, Value)>) {
    fake_gateway_answering(
        move |_, _| json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] }),
    )
}

/// A loopback gateway whose System One endpoint picks `choice` with 0.8 and
/// spreads the rest over the other offered options. Chat gets 404.
fn fake_gateway_deciding(choice: &'static str) -> (String, mpsc::Receiver<(String, Value)>) {
    fake_gateway_answering(move |path, body| {
        if path != "/v1/systemone" {
            return json!({ "error": "not_found" });
        }
        let criteria = body["questions"]["action"]["criteria"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let rest = criteria.len().saturating_sub(1).max(1) as f64;
        let probabilities: Map<String, Value> = criteria
            .keys()
            .map(|key| {
                let p = if key == choice { 0.8 } else { 0.2 / rest };
                (key.clone(), json!(p))
            })
            .collect();
        json!({
            "model": "apofasi-lexical-0.1.1",
            "answers": { "action": {
                "type": "choice",
                "choice": choice,
                "confidence": 0.8,
                "probabilities": probabilities
            } }
        })
    })
}

fn fake_gateway_answering(
    reply: impl Fn(&str, &Value) -> Value + Send + 'static,
) -> (String, mpsc::Receiver<(String, Value)>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (seen, requests) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buf = Vec::new();
            let mut chunk = [0_u8; 4096];
            let (head, body_len) = loop {
                let Ok(n) = stream.read(&mut chunk) else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(at) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..at]).into_owned();
                    let len = head
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())?
                        })
                        .unwrap_or(0);
                    break (head, at + 4 + len);
                }
            };
            while buf.len() < body_len {
                let Ok(n) = stream.read(&mut chunk) else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let authorization = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("authorization")
                        .then(|| value.trim().to_owned())
                })
                .unwrap_or_default();
            let at = head.len() + 4;
            let body: Value = serde_json::from_slice(&buf[at..body_len]).unwrap_or(Value::Null);
            let path = head
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_owned();
            let answer = reply(&path, &body);
            let status = if answer.get("error").is_some() {
                "404 Not Found"
            } else {
                "200 OK"
            };
            let _ = seen.send((authorization, body));
            let reply = answer.to_string().into_bytes();
            let _ = write!(
                stream,
                "HTTP/1.0 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                reply.len()
            );
            let _ = stream.write_all(&reply);
        }
    });
    (origin, requests)
}

/// `a3s-box run` without the MicroVM: the box's own `launch.cjs` runs on the
/// host with the `-e` environment, unpacks the package it carries, and starts
/// the package entry behind the published port.
const LAUNCHER_BOX: &str = r#"#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs'
const args = process.argv.slice(2)
const here = (name) => new URL(name, import.meta.url)
appendFileSync(here('./args.log'), args.join('\0') + '\n')
appendFileSync(here('./env.log'), JSON.stringify(process.env) + '\n')
if (args[0] === 'image-inspect') process.exit(0)
if (args[0] === 'rm') {
  try { process.kill(Number(readFileSync(here('./launcher.pid'), 'utf8'))) } catch {}
  process.exit(0)
}
const env = { PATH: process.env.PATH }
let launcher
let root
let port
for (let index = 0; index < args.length; index++) {
  if (args[index] === '-e') {
    const [name, ...value] = args[++index].split('=')
    env[name] = value.join('=')
  } else if (args[index] === '-v') {
    const [host, guest] = args[++index].split(':')
    if (guest === '/a3s/edge/launch.cjs') launcher = host
    if (guest === '/a3s/agent') root = host
  } else if (args[index] === '-p') {
    port = args[++index].split(':')[0]
  }
}
env.A3S_EDGE_PROXY_PORT = port
if (root && env.A3S_EDGE_ENTRY.startsWith('/a3s/agent/')) {
  env.A3S_EDGE_ENTRY = root + env.A3S_EDGE_ENTRY.slice('/a3s/agent'.length)
}
const child = spawn(process.execPath, [launcher], { env, stdio: ['ignore', 'inherit', 'inherit'] })
writeFileSync(here('./launcher.pid'), String(child.pid))
process.stdout.write('Box fake_box (5f7eafd3b0e3) started.\n')
child.on('exit', (code) => process.exit(code ?? 0))
"#;

fn copy_published(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_str(), Some("tests" | "tools" | "brand")) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            copy_published(&path, &to.join(&name))?;
        } else {
            std::fs::copy(&path, to.join(&name))?;
        }
    }
    Ok(())
}

fn observation(options: Value, baseline: &str) -> ViewTurn {
    let body = json!({
        "prompt": "选择下一步动作。",
        "parameters": { "observation": {
            "task": "把方块放到目标区，然后回到初始位。",
            "decision": 2,
            "tcp": [0.45, -0.1, 0.13],
            "cube": [0.45, -0.1, 0.025],
            "target": [0.3, 0.25, 0.0],
            "gripper": "open",
            "holding": false,
            "cube_on_target": false,
            "options": options,
            "baseline_choice": baseline
        } }
    });
    ViewTurn::parse(body.to_string().as_bytes()).unwrap()
}

// Catch a published robot workbench agent failing inside the edge launcher, losing
// the warm box between decisions, or a model reply that is not the chosen action.
#[test]
fn the_cobot_arm_agent_decides_in_one_warm_box_through_the_real_launcher() {
    workbench_agent_decides_in_one_warm_box("cobot-arm");
}

#[test]
fn the_g1_arm_agent_decides_in_one_warm_box_through_the_real_launcher() {
    workbench_agent_decides_in_one_warm_box("g1-arm");
}

#[test]
fn the_iris_agent_decides_in_one_warm_box_through_the_real_launcher() {
    workbench_agent_decides_in_one_warm_box("iris");
}

fn workbench_agent_decides_in_one_warm_box(name: &str) {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../use-registry/packages")
        .join(name);
    if !package.join("agent/run.mjs").is_file() {
        return;
    }
    let Some(lab) = Lab::new(name) else {
        return;
    };
    let installed = lab.dir.join("installed");
    copy_published(&package, &installed).unwrap();
    std::fs::write(lab.dir.join("a3s-box.mjs"), LAUNCHER_BOX).unwrap();
    let agent = agent_at(installed.join("agent/run.mjs"));
    let (origin, requests) = fake_gateway_deciding("go_home");
    let signed_in = ModelAccess {
        origin: origin.clone(),
        bearer: Some(BEARER.to_owned()),
    };
    let signed_out = ModelAccess {
        origin,
        bearer: None,
    };
    let token = "4".repeat(64);
    let two = json!({ "descend_to_cube": "下降到抓取高度", "go_home": "回到初始位" });

    let started = Instant::now();
    let chosen = lab
        .episodes
        .turn(
            &token,
            "lab",
            &agent,
            &observation(two.clone(), "descend_to_cube"),
            &signed_in,
        )
        .unwrap();
    let cold = started.elapsed();
    let chosen: Value = serde_json::from_str(&chosen).unwrap();
    assert_eq!(chosen["choice"], "go_home", "{chosen}");
    assert_eq!(chosen["source"], "model");
    assert_eq!(chosen["model"], "apofasi-lexical-0.1.1");
    assert_eq!(chosen["probabilities"]["go_home"], 0.8);
    assert_eq!(chosen["probabilities"]["descend_to_cube"], 0.2);
    let (authorization, body) = requests.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(authorization, format!("Bearer {BEARER}"));
    assert_eq!(body["model"], "platform/apofasi");
    assert_eq!(body["questions"]["action"]["type"], "choice");
    assert_eq!(
        body["questions"]["action"]["criteria"]["descend_to_cube"],
        "下降到抓取高度"
    );
    assert!(body.get("messages").is_none(), "{body}");

    let started = Instant::now();
    let fallback = lab
        .episodes
        .turn(
            &token,
            "lab",
            &agent,
            &observation(two, "descend_to_cube"),
            &signed_out,
        )
        .unwrap();
    let warm = started.elapsed();
    let fallback: Value = serde_json::from_str(&fallback).unwrap();
    assert_eq!(fallback["choice"], "descend_to_cube", "{fallback}");
    assert_eq!(fallback["source"], "rule");
    assert_eq!(fallback["note"], "model unavailable (401)");
    assert!(requests.recv_timeout(Duration::from_millis(300)).is_err());

    let single = lab
        .episodes
        .turn(
            &token,
            "lab",
            &agent,
            &observation(json!({ "go_home": "回到初始位" }), "go_home"),
            &signed_in,
        )
        .unwrap();
    let single: Value = serde_json::from_str(&single).unwrap();
    assert_eq!(single["choice"], "go_home");
    assert_eq!(single["note"], "only one option");
    assert!(requests.recv_timeout(Duration::from_millis(300)).is_err());

    assert_eq!(lab.runs(), 1, "{}", lab.log("args.log"));
    assert!(!lab.log("args.log").contains(BEARER));
    assert!(!lab.log("env.log").contains(BEARER));
    assert!(warm < Duration::from_secs(1), "warm turn took {warm:?}");
    eprintln!("{name} edge turns: cold {cold:?}, warm {warm:?}");
    lab.episodes.end_view(&token);
    assert!(lab.removed(), "{}", lab.log("args.log"));
    assert!(!lab.episodes.has_episodes());
}

// Catch a published robot workbench agent failing in a real MicroVM: the box the
// host resolves, the default image, and the brokered model call.
#[test]
#[ignore = "set A3S_STUDIO_BOX_LIVE=1 with a3s-box installed"]
fn the_cobot_arm_agent_decides_in_a_live_a3s_box() {
    workbench_agent_decides_in_a_live_a3s_box("cobot-arm");
}

#[test]
#[ignore = "set A3S_STUDIO_BOX_LIVE=1 with a3s-box installed"]
fn the_g1_arm_agent_decides_in_a_live_a3s_box() {
    workbench_agent_decides_in_a_live_a3s_box("g1-arm");
}

#[test]
#[ignore = "set A3S_STUDIO_BOX_LIVE=1 with a3s-box installed"]
fn the_iris_agent_decides_in_a_live_a3s_box() {
    workbench_agent_decides_in_a_live_a3s_box("iris");
}

fn workbench_agent_decides_in_a_live_a3s_box(name: &str) {
    if std::env::var_os("A3S_STUDIO_BOX_LIVE").is_none() {
        eprintln!("{name} live a3s-box test skipped: A3S_STUDIO_BOX_LIVE is unset");
        return;
    }
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../use-registry/packages")
        .join(name);
    let dir = std::env::temp_dir().join(format!("a3s-aix-live-{}", uuid::Uuid::now_v7()));
    let installed = dir.join("installed");
    copy_published(&package, &installed).unwrap();
    let episodes = AixEpisodes::with_box(None, None, IDLE_LIMIT).unwrap();
    let agent = agent_at(installed.join("agent/run.mjs"));
    let (origin, requests) = fake_gateway_deciding("go_home");
    let model = ModelAccess {
        origin,
        bearer: Some(BEARER.to_owned()),
    };
    let token = "5".repeat(64);
    let two = json!({ "descend_to_cube": "下降到抓取高度", "go_home": "回到初始位" });
    let boxes_before = edge_boxes();

    let started = Instant::now();
    let cold = episodes.turn(
        &token,
        "lab",
        &agent,
        &observation(two.clone(), "descend_to_cube"),
        &model,
    );
    let cold_time = started.elapsed();
    let started = Instant::now();
    let warm = episodes.turn(
        &token,
        "lab",
        &agent,
        &observation(two, "descend_to_cube"),
        &model,
    );
    let warm_time = started.elapsed();
    // `shutdown` waits for `rm -f`; `end_view` would leave it to a task this test outlives.
    episodes.shutdown();
    let _ = std::fs::remove_dir_all(&dir);

    for reply in [cold.unwrap(), warm.unwrap()] {
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["choice"], "go_home", "{reply}");
        assert_eq!(reply["source"], "model", "{reply}");
    }
    for _ in 0..2 {
        let (authorization, _) = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(authorization, format!("Bearer {BEARER}"));
    }
    eprintln!("live a3s-box {name} turns: cold {cold_time:?}, warm {warm_time:?}");
    assert_eq!(edge_boxes(), boxes_before, "a box was left behind");
}

fn edge_boxes() -> usize {
    let listed = std::process::Command::new("a3s-box")
        .args(["ps", "-a"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| line.contains(":18080"))
        .count()
}

fn session_of(text: &str) -> &str {
    text.split(':').nth(1).unwrap_or("")
}

// Catch a cold MicroVM per call, a lost session, or the member token reaching the box.
#[test]
fn a_warm_box_serves_many_turns_and_brokers_the_model_without_the_token() {
    let Some(lab) = Lab::new("warm") else { return };
    let (origin, requests) = fake_gateway();
    let model = ModelAccess {
        origin,
        bearer: Some(BEARER.to_owned()),
    };
    let token = "1".repeat(64);
    let first = lab
        .episodes
        .turn(&token, "lab", &lab.agent, &prompt("first"), &model)
        .unwrap();
    let second = lab
        .episodes
        .turn(&token, "lab", &lab.agent, &prompt("second"), &model)
        .unwrap();
    assert!(
        first.starts_with("1:") && first.ends_with(":200:hold"),
        "{first}"
    );
    assert!(
        second.starts_with("2:") && second.ends_with(":200:hold"),
        "{second}"
    );
    assert_eq!(session_of(&first), session_of(&second));
    assert_eq!(lab.runs(), 1, "{}", lab.log("args.log"));
    for expected in ["first", "second"] {
        let (authorization, body) = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(authorization, format!("Bearer {BEARER}"));
        assert_eq!(body["model"], "gw-model");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["content"], expected);
    }
    assert!(!lab.log("args.log").contains(BEARER));
    assert!(!lab.log("env.log").contains(BEARER));
    lab.episodes.end_view(&token);
    assert!(lab.removed(), "{}", lab.log("args.log"));
    assert!(!lab.episodes.has_episodes());
}

fn jpeg(fill: u8, len: usize) -> String {
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0];
    bytes.resize(len, fill);
    format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes))
}

fn png(len: usize) -> String {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.resize(len, 7);
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

fn turn_with(images: Value) -> Option<ViewTurn> {
    ViewTurn::parse(
        json!({ "prompt": "look", "images": images })
            .to_string()
            .as_bytes(),
    )
}

// Catch image bytes entering the box, or the gateway missing the images a box referenced.
#[test]
fn turn_images_reach_the_gateway_by_reference_and_never_enter_the_box() {
    let Some(lab) = Lab::new("images") else {
        return;
    };
    let (origin, requests) = fake_gateway();
    let model = ModelAccess {
        origin,
        bearer: Some(BEARER.to_owned()),
    };
    let token = "6".repeat(64);
    let external = jpeg(1, 4096);
    let wrist = png(2048);
    let turn = turn_with(json!([
        { "name": "external", "data": external },
        { "name": "wrist", "data": wrist }
    ]))
    .unwrap();
    let reply = lab
        .episodes
        .turn(&token, "lab", &lab.agent, &turn, &model)
        .unwrap();
    assert!(reply.ends_with(":200:hold"), "{reply}");

    let (authorization, body) = requests.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(authorization, format!("Bearer {BEARER}"));
    let content = &body["messages"][0]["content"];
    assert_eq!(content[0], json!({ "type": "text", "text": "look" }));
    assert_eq!(content[1]["image_url"]["url"], external);
    assert_eq!(content[2]["image_url"]["url"], wrist);

    let commands = lab.log("commands.log");
    let command: Value = serde_json::from_str(commands.lines().next().unwrap()).unwrap();
    assert_eq!(
        command["images"],
        json!([
            { "name": "external", "media_type": "image/jpeg", "bytes": 4096 },
            { "name": "wrist", "media_type": "image/png", "bytes": 2048 }
        ])
    );
    assert!(!commands.contains("base64"), "{commands}");
    assert!(!lab.log("args.log").contains("base64"));

    let plain = lab
        .episodes
        .turn(&token, "lab", &lab.agent, &prompt("again"), &model)
        .unwrap();
    assert!(plain.ends_with(":200:hold"), "{plain}");
    let (_, body) = requests.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(body["messages"][0]["content"], "again");
    assert_eq!(lab.runs(), 1, "{}", lab.log("args.log"));
    lab.episodes.end_view(&token);
}

// Catch a view smuggling a remote URL, an unknown type, or an oversize image into a turn.
#[test]
fn turn_images_accept_only_two_named_jpeg_or_png_frames() {
    let one = turn_with(json!([{ "name": "external", "data": jpeg(0, 64) }])).unwrap();
    assert_eq!(one.images.len(), 1);
    assert_eq!(one.images[0].media_type, "image/jpeg");
    assert!(!format!("{one:?}").contains("base64"));
    assert!(turn_with(json!([{ "name": "wrist_2", "data": png(MAX_TURN_IMAGE) }])).is_some());
    let gif = format!("data:image/gif;base64,{}", STANDARD.encode(b"GIF89a....."));
    let mislabelled = format!(
        "data:image/png;base64,{}",
        STANDARD.encode([0xff, 0xd8, 0xff, 0])
    );
    for images in [
        json!([{ "name": "a", "data": jpeg(0, 64) }, { "name": "b", "data": jpeg(0, 64) }, { "name": "c", "data": jpeg(0, 64) }]),
        json!([{ "name": "a", "data": jpeg(0, 64) }, { "name": "a", "data": jpeg(1, 64) }]),
        json!([{ "name": "External", "data": jpeg(0, 64) }]),
        json!([{ "name": "1st", "data": jpeg(0, 64) }]),
        json!([{ "name": "x".repeat(MAX_IMAGE_NAME + 1), "data": jpeg(0, 64) }]),
        json!([{ "name": "a", "data": jpeg(0, MAX_TURN_IMAGE + 1) }]),
        json!([{ "name": "a", "data": "https://example.com/cube.jpg" }]),
        json!([{ "name": "a", "data": gif }]),
        json!([{ "name": "a", "data": mislabelled }]),
        json!([{ "name": "a", "data": "data:image/jpeg;base64,!!!!" }]),
        json!([{ "name": "a", "data": jpeg(0, 64), "url": "x" }]),
        json!({ "name": "a", "data": jpeg(0, 64) }),
    ] {
        assert!(turn_with(images.clone()).is_none(), "{images}");
    }
}

// Catch a box reaching a remote image URL, inlining its own bytes, or replaying an image.
#[test]
fn chat_bodies_take_turn_images_only_by_reference() {
    let turn = turn_with(json!([
        { "name": "external", "data": jpeg(3, 128) },
        { "name": "wrist", "data": png(64) }
    ]))
    .unwrap();
    let image = |url: &str| json!({ "type": "image_url", "image_url": { "url": url } });
    let body =
        |parts: Value| json!({ "model": "m", "messages": [{ "role": "user", "content": parts }] });

    let sent = body(json!([
        { "type": "text", "text": "Camera view: external" },
        { "type": "image_url", "image_url": { "url": "aix-image:external", "detail": "high" } },
        image("aix-image:wrist")
    ]));
    let checked: Value =
        serde_json::from_slice(&model_body(Some(&sent), &turn.images).unwrap()).unwrap();
    let parts = &checked["messages"][0]["content"];
    assert_eq!(parts[1]["image_url"]["url"], turn.images[0].url);
    assert_eq!(parts[1]["image_url"]["detail"], "high");
    assert_eq!(parts[2]["image_url"]["url"], turn.images[1].url);
    assert_eq!(checked["stream"], false);

    for (parts, reason) in [
        (
            json!([image("aix-image:external"), image("aix-image:external")]),
            "image_reused",
        ),
        (json!([image("aix-image:depth")]), "unsupported_image"),
        (
            json!([image("https://example.com/cube.jpg")]),
            "unsupported_image",
        ),
        (json!([image(&jpeg(0, 64))]), "unsupported_image"),
        (
            json!([{ "type": "image_url", "image_url": { "url": "aix-image:wrist", "detail": "max" } }]),
            "unsupported_image",
        ),
        (
            json!([{ "type": "image_url", "image_url": "aix-image:wrist" }]),
            "unsupported_image",
        ),
        (
            json!([{ "type": "input_audio", "input_audio": { "data": "", "format": "wav" } }]),
            "unsupported_content",
        ),
        (
            json!([{ "type": "text", "text": "x", "cache_control": {} }]),
            "unsupported_content",
        ),
        (json!(["bare"]), "unsupported_content"),
    ] {
        assert_eq!(
            model_body(Some(&body(parts.clone())), &turn.images),
            Err(reason),
            "{parts}"
        );
    }
    assert_eq!(
        model_body(Some(&body(json!([image("aix-image:external")]))), &[]),
        Err("unsupported_image")
    );
}

// Catch a failed turn leaving a poisoned box warm, or a signed-out member reaching the gateway.
#[test]
fn a_failed_turn_removes_the_box_and_a_signed_out_model_call_is_refused_in_the_box() {
    let Some(lab) = Lab::new("failed") else {
        return;
    };
    let (origin, requests) = fake_gateway();
    let model = ModelAccess {
        origin,
        bearer: None,
    };
    let token = "2".repeat(64);
    let refused = lab
        .episodes
        .turn(&token, "lab", &lab.agent, &prompt("hello"), &model)
        .unwrap();
    assert!(refused.ends_with(":401:unauthorized"), "{refused}");
    assert!(requests.recv_timeout(Duration::from_millis(300)).is_err());
    let failed = lab
        .episodes
        .turn(&token, "lab", &lab.agent, &prompt("fail"), &model)
        .unwrap_err();
    assert!(
        matches!(&failed, EpisodeError::Failed(message) if message.contains("the package refused")),
        "{failed:?}"
    );
    assert!(lab.removed(), "{}", lab.log("args.log"));
    assert!(!lab.episodes.has_episodes());
}

// Catch a dead or idle view keeping its MicroVM.
#[test]
fn the_reaper_ends_boxes_whose_view_no_longer_binds_the_agent() {
    let Some(lab) = Lab::new("reap") else { return };
    let (origin, _requests) = fake_gateway();
    let model = ModelAccess {
        origin,
        bearer: Some(BEARER.to_owned()),
    };
    let token = "3".repeat(64);
    lab.episodes
        .turn(&token, "lab", &lab.agent, &prompt("first"), &model)
        .unwrap();
    lab.episodes.reap(|_, _| true);
    assert!(lab.episodes.has_episodes());
    lab.episodes
        .reap(|view, agent| view != token || agent != "lab");
    assert!(lab.removed(), "{}", lab.log("args.log"));
    assert!(!lab.episodes.has_episodes());
}

// Catch overlapping turns on one box or unbounded warm boxes.
#[test]
fn a_running_turn_and_the_box_limit_refuse_without_starting_a_box() {
    let episodes = AixEpisodes::with_box(
        Some(PathBuf::from("/nonexistent/a3s-box")),
        None,
        IDLE_LIMIT,
    )
    .unwrap();
    let agent = agent_at(PathBuf::from("/nonexistent/agent/run.mjs"));
    let model = ModelAccess {
        origin: "http://127.0.0.1:9".to_owned(),
        bearer: None,
    };
    let held = episodes.episode(&"a".repeat(64), "lab", &agent).unwrap();
    let guard = held.warm.try_lock().unwrap();
    assert_eq!(
        episodes.turn(&"a".repeat(64), "lab", &agent, &prompt("x"), &model),
        Err(EpisodeError::Busy)
    );
    drop(guard);
    episodes.episode(&"b".repeat(64), "lab", &agent).unwrap();
    assert_eq!(
        episodes
            .turn(&"c".repeat(64), "lab", &agent, &prompt("x"), &model)
            .unwrap_err(),
        EpisodeError::Capacity
    );
    episodes.end_view(&"b".repeat(64));
    assert!(matches!(
        episodes.turn(&"c".repeat(64), "lab", &agent, &prompt("x"), &model),
        Err(EpisodeError::Failed(_))
    ));
    assert_eq!(episodes.episodes().len(), 1);
}

// Catch a view smuggling fields, streaming, or oversize payloads to the host.
#[test]
fn view_turns_and_model_bodies_accept_only_the_closed_shape() {
    assert!(ViewTurn::parse(br#"{"prompt":"go","parameters":{"step":1}}"#).is_some());
    for body in [
        br#"{"prompt":""}"#.to_vec(),
        br#"{"prompt":"go","extra":1}"#.to_vec(),
        br#"{"prompt":"go","parameters":[1]}"#.to_vec(),
        json!({ "prompt": "x".repeat(MAX_PROMPT + 1) })
            .to_string()
            .into_bytes(),
    ] {
        assert!(
            ViewTurn::parse(&body).is_none(),
            "{}",
            String::from_utf8_lossy(&body)
        );
    }
    let messages = json!([{ "role": "user", "content": "hi" }]);
    let ok = model_body(Some(&json!({ "model": "m", "messages": messages })), &[]).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&ok).unwrap()["stream"],
        false
    );
    for (body, reason) in [
        (
            json!({ "model": "m", "messages": messages, "stream": true }),
            "streaming_unsupported",
        ),
        (
            json!({ "model": "m", "messages": messages, "api_key": "x" }),
            "unsupported_field",
        ),
        (json!({ "messages": messages }), "invalid_model"),
        (json!({ "model": "m", "messages": [] }), "invalid_messages"),
    ] {
        assert_eq!(model_body(Some(&body), &[]), Err(reason));
    }
    let quiet = json!({ "model": "m", "messages": messages, "reasoning_effort": "none" });
    assert!(model_body(Some(&quiet), &[]).is_ok());
    for effort in [
        json!("off"),
        json!("None"),
        json!(0),
        json!(false),
        Value::Null,
    ] {
        let body = json!({ "model": "m", "messages": messages, "reasoning_effort": effort });
        assert!(
            model_body(Some(&body), &[])
                .unwrap_err()
                .starts_with("invalid_reasoning_effort: expected none,")
        );
    }

    assert_eq!(ModelApi::parse(None), Ok(ModelApi::ChatCompletions));
    assert_eq!(
        ModelApi::parse(Some(&json!("systemone"))),
        Ok(ModelApi::SystemOne)
    );
    for api in [json!("embeddings"), json!(1), Value::Null] {
        assert_eq!(
            ModelApi::parse(Some(&api)),
            Err("unsupported_api: expected chat or systemone")
        );
    }
    let questions = json!({ "action": { "type": "choice", "criteria": { "a": null, "b": null } } });
    let decision = json!({ "model": "platform/apofasi", "state": { "tcp": [0, 0, 0] }, "questions": questions });
    assert_eq!(
        serde_json::from_slice::<Value>(&decision_body(Some(&decision)).unwrap()).unwrap(),
        decision
    );
    for (body, reason) in [
        (
            json!({ "model": "m", "state": "s", "questions": questions, "messages": [] }),
            "unsupported_field",
        ),
        (
            json!({ "state": "s", "questions": questions }),
            "invalid_model",
        ),
        (
            json!({ "model": "m", "state": 1, "questions": questions }),
            "invalid_state",
        ),
        (
            json!({ "model": "m", "state": "s", "questions": {} }),
            "invalid_questions",
        ),
        (
            json!({ "model": "m", "state": "s", "questions": { "q": "x" } }),
            "invalid_questions",
        ),
        (
            json!({ "model": "m", "state": "x".repeat(MAX_MODEL_BODY), "questions": questions }),
            "body_too_large",
        ),
    ] {
        assert_eq!(decision_body(Some(&body)), Err(reason));
    }
    assert_eq!(
        dechunk(b"4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n").unwrap(),
        b"{\"a\":1}"
    );
    assert!(dechunk(b"9\r\nshort\r\n").is_none());
    assert!(not_delivered("connection reset: os error 54"));
    assert!(!not_delivered("agent runtime refused the command (500)"));
}
