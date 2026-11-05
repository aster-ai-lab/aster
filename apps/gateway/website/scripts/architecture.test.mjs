import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import {
  PlaybackClock,
  STEP_DURATION,
  SYSTEM_EDGES,
  SYSTEM_STAGES,
  DIAGRAM_NODES,
  DIAGRAM_SIZE,
  edgeWaypoints,
  frameAt,
} from "../architecture/architecture.mjs";
import { CAPABILITY_GROUPS, CAPABILITY_STATES } from "../architecture/capabilities.mjs";

const edge = (id) => SYSTEM_EDGES.find((item) => item.id === id);

test("pause excludes hidden time and resume preserves the position", () => {
  const clock = new PlaybackClock();
  clock.play(100);
  clock.pause(350);
  assert.equal(clock.time(5000), 250);
  clock.play(5000);
  clock.play(5050);
  assert.equal(clock.time(5100), 350);
});

test("seeking and the one timeline have deterministic stage boundaries", () => {
  const clock = new PlaybackClock();
  clock.seek(STEP_DURATION * 3, 0);
  assert.equal(frameAt(clock.time(1000)).step, 3);
  clock.play(1000);
  clock.seek(STEP_DURATION, 2000);
  assert.equal(frameAt(clock.time(2100)).step, 1);
  assert.equal(frameAt(STEP_DURATION - 1).step, 0);
  assert.equal(frameAt(STEP_DURATION * SYSTEM_STAGES.length).step, 0);
});

test("every drawn module has unique source-backed metadata and every wire resolves", () => {
  const nodes = new Set();
  for (const [id, x, y, kind] of DIAGRAM_NODES) {
    assert.ok(!nodes.has(id), id);
    nodes.add(id);
    assert.ok(x >= 60 && x <= DIAGRAM_SIZE.width - 60, id);
    assert.ok(y >= 60 && y <= DIAGRAM_SIZE.height - 60, id);
    assert.ok(kind >= 0 && kind <= 3, id);
  }
  assert.deepEqual([...nodes].sort(), CAPABILITY_GROUPS.map((group) => group.id).sort());
  const edges = new Set();
  for (const item of SYSTEM_EDGES) {
    assert.ok(!edges.has(item.id), item.id);
    edges.add(item.id);
    assert.ok(nodes.has(item.from) && nodes.has(item.to), item.id);
  }
  for (const stage of SYSTEM_STAGES)
    assert.ok(
      stage.edges.every((id) => edges.has(id)),
      stage.title,
    );
});

test("each explanation points to an existing repository source and valid feature states", () => {
  const root = fileURLToPath(new URL("../../../../", import.meta.url));
  const ids = new Set();
  for (const group of CAPABILITY_GROUPS) {
    assert.ok(group.owner && group.description && group.title && group.subtitle, group.id);
    assert.ok(existsSync(join(root, group.source)), group.source);
    for (const [id, name, state] of group.features) {
      assert.ok(!ids.has(id), id);
      ids.add(id);
      assert.ok(name && Object.hasOwn(CAPABILITY_STATES, state), id);
    }
  }
});

test("all caller traffic enters exactly one logical Gateway entry layer", () => {
  const outgoing = SYSTEM_EDGES.filter((item) => item.from === "callers");
  assert.equal(outgoing.length, 1);
  assert.equal(outgoing[0].to, "entrypoints");
  assert.equal(edge("entry-http").from, "entrypoints");
  assert.equal(edge("entry-transport").from, "entrypoints");
});

test("the layout reads forward through shared stages, with model selection above and responses below", () => {
  const position = new Map(DIAGRAM_NODES.map(([id, x, y]) => [id, { x, y }]));
  const main = [
    "callers",
    "entrypoints",
    "http-route",
    "middleware",
    "backend-select",
    "proxy",
    "upstreams",
  ];
  for (let i = 1; i < main.length; i += 1) {
    assert.ok(position.get(main[i - 1]).x < position.get(main[i]).x);
    assert.equal(position.get(main[i]).y, position.get(main[0]).y);
  }
  assert.ok(position.get("middleware").x < position.get("inference").x);
  assert.ok(position.get("inference").x < position.get("model-policy").x);
  assert.ok(position.get("model-policy").x < position.get("backend-select").x);
  assert.ok(position.get("model-policy").y < position.get("middleware").y);
  assert.ok(position.get("transport").y > position.get("middleware").y);
  assert.ok(position.get("response").y > position.get("transport").y);
  assert.ok(position.get("compile").y < position.get("inference").y);
  assert.ok(position.get("agent-cli").y > position.get("response").y);
  for (const id of position.keys())
    assert.ok(
      SYSTEM_EDGES.some((item) => !item.detailOnly && (item.from === id || item.to === id)),
      `${id} must retain a connection outside hover`,
    );
});

test("each connector has bounded orthogonal nonzero segments for rounded paths and direction arrows", () => {
  for (const item of SYSTEM_EDGES) {
    const route = edgeWaypoints(item);
    assert.ok(route.length >= 2, item.id);
    for (const [x, y] of route) {
      assert.ok(Number.isFinite(x) && x >= 0 && x <= DIAGRAM_SIZE.width, item.id);
      assert.ok(Number.isFinite(y) && y >= 0 && y <= DIAGRAM_SIZE.height, item.id);
    }
    for (let i = 1; i < route.length; i += 1) {
      const [x, y] = route[i],
        [previousX, previousY] = route[i - 1];
      assert.ok((x === previousX) !== (y === previousY), item.id);
    }
  }
});

test("HTTP and inference are ordered separately from raw transport", () => {
  assert.deepEqual(
    [edge("entry-http").to, edge("http-middleware").to, edge("middleware-inference").to],
    ["http-route", "middleware", "inference"],
  );
  assert.equal(edge("inference-policy").to, "model-policy");
  assert.equal(edge("policy-backend").to, "backend-select");
  assert.equal(edge("ordinary-backend").from, "middleware");
  assert.equal(edge("ordinary-backend").to, "backend-select");
  assert.equal(edge("raw-upstream").from, "transport");
  assert.equal(edge("raw-upstream").to, "upstreams");
  assert.ok(!SYSTEM_EDGES.some((item) => item.from === "transport" && item.to === "middleware"));
});

test("management, Studio delegation and Agent CLI retain distinct authority", () => {
  assert.equal(edge("management-build").kind, "control");
  assert.equal(edge("snapshot-entry").kind, "control");
  assert.equal(edge("proxy-modelcontrol").to, "model-control");
  assert.equal(edge("proxy-modelcontrol").secondary, true);
  const cli = SYSTEM_EDGES.filter(
    (item) =>
      item.from === "agent-cli" ||
      item.to === "agent-cli" ||
      item.from === "native-cli" ||
      item.to === "native-cli",
  );
  assert.equal(cli.length, 1);
  assert.equal(cli[0].kind, "operations");
  assert.equal(cli[0].to, "native-cli");
});

test("reserved standards and unavailable Harness orchestration are explanations, not fake runtime paths", () => {
  assert.ok(
    !DIAGRAM_NODES.some(([id]) =>
      ["harness-routing", "harness-native", "mcp", "tools"].includes(id),
    ),
  );
  const features = new Map(
    CAPABILITY_GROUPS.flatMap((group) =>
      group.features.map(([id, name, state]) => [id, { name, state }]),
    ),
  );
  assert.equal(features.get("reserved-standards").state, "reserved");
  assert.equal(features.get("no-harness-router").state, "absent");
  assert.equal(features.get("audio-scope").state, "separate");
});

test("request chronology authenticates before middleware and begins durable usage before dispatch", () => {
  const phase = (order) => SYSTEM_STAGES.find((stage) => stage.order === order);
  assert.deepEqual(
    SYSTEM_STAGES.map((stage) => stage.order),
    ["1", "2", "3", "4", "5", "6", "7", "R", "8"],
  );
  assert.deepEqual(phase("2").edges, ["entry-http"]);
  assert.deepEqual(phase("3").edges, ["http-middleware"]);
  assert.deepEqual(phase("6").flow, [["policy-backend"], ["usage-begin"]]);
  assert.deepEqual(phase("7").flow, [["backend-proxy"], ["proxy-upstream"]]);
  assert.equal(phase("R").failed, "upstreams");
  assert.ok(phase("R").edges.indexOf("usage-begin") < phase("R").edges.indexOf("proxy-upstream"));
  assert.deepEqual(phase("8").flow, [["upstream-response"], ["response-caller", "response-usage"]]);
  assert.ok(!SYSTEM_STAGES.some((stage) => stage.edges.includes("usage-cloud")));
});

test("playback advances hops causally while response delivery and observation can overlap", () => {
  const dispatch = SYSTEM_STAGES.findIndex((stage) => stage.order === "7");
  const first = frameAt(STEP_DURATION * (dispatch + 0.25));
  assert.deepEqual(first.stage.flow[first.hop], ["backend-proxy"]);
  assert.equal(first.hopProgress, 0.5);
  const next = frameAt(STEP_DURATION * (dispatch + 0.75));
  assert.deepEqual(next.stage.flow[next.hop], ["proxy-upstream"]);
  assert.equal(next.hopProgress, 0.5);
  const response = SYSTEM_STAGES.findIndex((stage) => stage.order === "8");
  const body = frameAt(STEP_DURATION * (response + 0.75));
  assert.deepEqual(body.stage.flow[body.hop], ["response-caller", "response-usage"]);
});
