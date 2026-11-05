import { CAPABILITY_GROUPS, CAPABILITY_STATES } from "./capabilities.mjs";

export const STEP_DURATION = 5000;
export const DIAGRAM_SIZE = { width: 1620, height: 1100 };
export const DIAGRAM_NODES = [
  ["callers", 90, 490, 0],
  ["entrypoints", 270, 490, 1],
  ["http-route", 450, 490, 2],
  ["middleware", 630, 490, 2],
  ["inference", 810, 330, 2],
  ["model-policy", 990, 330, 2],
  ["backend-select", 1170, 490, 2],
  ["proxy", 1350, 490, 2],
  ["upstreams", 1530, 490, 3],
  ["response", 1350, 830, 2],
  ["usage", 990, 830, 2],
  ["configuration", 270, 90, 2],
  ["compile", 630, 90, 2],
  ["snapshot", 990, 90, 2],
  ["management", 450, 90, 2],
  ["local-ingress", 450, 270, 2],
  ["local-response", 810, 620, 2],
  ["demand", 630, 270, 2],
  ["transport", 450, 720, 2],
  ["model-control", 1530, 290, 2],
  ["agent-cli", 270, 1020, 2],
  ["native-cli", 630, 1020, 0],
  ["cloud-usage", 1530, 830, 2],
];

// Request, response, control dependencies and independent CLI operations are distinct.
export const SYSTEM_EDGES = [
  { id: "caller-entry", from: "callers", to: "entrypoints", kind: "request" },
  { id: "entry-http", from: "entrypoints", to: "http-route", kind: "request" },
  { id: "entry-transport", from: "entrypoints", to: "transport", kind: "transport" },
  {
    id: "entry-local",
    from: "entrypoints",
    to: "local-ingress",
    kind: "response",
    secondary: true,
  },
  { id: "http-middleware", from: "http-route", to: "middleware", kind: "request" },
  { id: "http-demand", from: "http-route", to: "demand", kind: "control", secondary: true },
  { id: "middleware-inference", from: "middleware", to: "inference", kind: "model" },
  { id: "inference-policy", from: "inference", to: "model-policy", kind: "model" },
  { id: "policy-backend", from: "model-policy", to: "backend-select", kind: "model" },
  { id: "ordinary-backend", from: "middleware", to: "backend-select", kind: "request" },
  { id: "backend-proxy", from: "backend-select", to: "proxy", kind: "request" },
  { id: "proxy-upstream", from: "proxy", to: "upstreams", kind: "request" },
  { id: "upstream-response", from: "upstreams", to: "response", kind: "response" },
  { id: "response-caller", from: "response", to: "callers", kind: "response" },
  { id: "response-usage", from: "response", to: "usage", kind: "response" },
  {
    id: "usage-begin",
    from: "backend-select",
    to: "usage",
    kind: "request",
    secondary: true,
    detailOnly: true,
  },
  { id: "usage-cloud", from: "usage", to: "cloud-usage", kind: "control", secondary: true },
  {
    id: "middleware-local",
    from: "middleware",
    to: "local-response",
    kind: "response",
    secondary: true,
  },
  {
    id: "local-response-return",
    from: "local-response",
    to: "response",
    kind: "response",
    secondary: true,
  },
  {
    id: "pre-route-return",
    from: "local-ingress",
    to: "callers",
    kind: "response",
    secondary: true,
  },
  { id: "raw-upstream", from: "transport", to: "upstreams", kind: "transport" },
  {
    id: "raw-return",
    from: "upstreams",
    to: "callers",
    kind: "transport",
    secondary: true,
    detailOnly: true,
  },
  { id: "config-build", from: "configuration", to: "compile", kind: "control" },
  { id: "management-build", from: "management", to: "compile", kind: "control" },
  { id: "build-snapshot", from: "compile", to: "snapshot", kind: "control" },
  { id: "snapshot-entry", from: "snapshot", to: "entrypoints", kind: "control", secondary: true },
  {
    id: "backend-transport",
    from: "backend-select",
    to: "transport",
    kind: "control",
    secondary: true,
    detailOnly: true,
  },
  { id: "proxy-modelcontrol", from: "proxy", to: "model-control", kind: "model", secondary: true },
  {
    id: "modelcontrol-upstream",
    from: "model-control",
    to: "upstreams",
    kind: "model",
    secondary: true,
  },
  { id: "cli-native", from: "agent-cli", to: "native-cli", kind: "operations" },
  {
    id: "retry-policy",
    from: "proxy",
    to: "model-policy",
    kind: "model",
    secondary: true,
    detailOnly: true,
  },
  {
    id: "direct-http",
    from: "http-route",
    to: "proxy",
    kind: "request",
    secondary: true,
    detailOnly: true,
  },
];

const nodePoints = new Map(DIAGRAM_NODES.map(([id, x, y]) => [id, { x, y }]));

export function edgeWaypoints(edge) {
  const a = nodePoints.get(edge.from),
    b = nodePoints.get(edge.to);
  const start = [a.x + 48, a.y],
    end = [b.x - 48, b.y];
  switch (edge.id) {
    case "entry-local":
      return [start, [350, a.y], [350, b.y], end];
    case "entry-transport":
      return [start, [360, a.y], [360, b.y], end];
    case "http-demand":
      return [start, [540, a.y], [540, b.y], end];
    case "middleware-inference":
    case "middleware-local":
      return [start, [720, a.y], [720, b.y], end];
    case "policy-backend":
      return [start, [1080, a.y], [1080, b.y], end];
    case "upstream-response":
      return [start, [1600, a.y], [1600, 780], [b.x, 780], [b.x, b.y - 42]];
    case "response-caller":
      return [
        [a.x - 48, a.y],
        [1260, a.y],
        [1260, 900],
        [20, 900],
        [20, b.y],
        [b.x - 48, b.y],
      ];
    case "response-usage":
      return [
        [a.x - 48, a.y],
        [b.x + 48, b.y],
      ];
    case "usage-begin":
      return [start, [1230, a.y], [1230, 580], [1060, 580], [1060, b.y], [b.x + 48, b.y]];
    case "usage-cloud":
      return [start, [1080, a.y], [1080, 970], [1440, 970], [1440, b.y], end];
    case "local-response-return":
      return [start, [900, a.y], [900, 780], [1260, 780], [1260, b.y], end];
    case "pre-route-return":
      return [
        [a.x - 48, a.y],
        [160, a.y],
        [160, 430],
        [b.x, 430],
        [b.x, b.y - 26],
      ];
    case "raw-upstream":
      return [start, [1470, a.y], [1470, b.y], end];
    case "raw-return":
      return [start, [1610, a.y], [1610, 940], [30, 940], [30, b.y], [b.x - 48, b.y]];
    case "config-build":
      return [start, [360, a.y], [360, 35], [b.x, 35], [b.x, b.y - 42]];
    case "snapshot-entry":
      return [
        [a.x - 48, a.y],
        [900, a.y],
        [900, 165],
        [b.x, 165],
        [b.x, b.y - 42],
      ];
    case "backend-transport":
      return [
        [a.x - 48, a.y],
        [1100, a.y],
        [1100, 660],
        [540, 660],
        [540, b.y],
        [b.x + 48, b.y],
      ];
    case "proxy-modelcontrol":
      return [start, [1440, a.y], [1440, b.y], end];
    case "modelcontrol-upstream":
      return [start, [1600, a.y], [1600, b.y], [b.x + 48, b.y]];
    case "retry-policy":
      return [
        [a.x, a.y - 42],
        [a.x, 210],
        [b.x, 210],
        [b.x, b.y - 42],
      ];
    case "direct-http":
      return [
        [a.x, a.y - 42],
        [a.x, 410],
        [b.x, 410],
        [b.x, b.y - 42],
      ];
    default:
      return [start, end];
  }
}

function roundedPath(points) {
  let path = `M${points[0].join(",")}`;
  for (let i = 1; i < points.length - 1; i += 1) {
    const [x, y] = points[i],
      previous = points[i - 1],
      next = points[i + 1];
    const incoming = Math.hypot(x - previous[0], y - previous[1]),
      outgoing = Math.hypot(next[0] - x, next[1] - y),
      radius = Math.min(10, incoming / 2, outgoing / 2);
    const before = [
        x - ((x - previous[0]) * radius) / incoming,
        y - ((y - previous[1]) * radius) / incoming,
      ],
      after = [x + ((next[0] - x) * radius) / outgoing, y + ((next[1] - y) * radius) / outgoing];
    path += ` L${before.join(",")} Q${x},${y} ${after.join(",")}`;
  }
  return `${path} L${points.at(-1).join(",")}`;
}

// Ordered hops illustrate causality, not measured runtime durations.
export const SYSTEM_STAGES = [
  {
    order: "1",
    title: "统一接入",
    text: "HTTP 请求进入接入层并取得当前本地运行快照。",
    flow: [["caller-entry"]],
    nodes: ["entrypoints"],
  },
  {
    order: "2",
    title: "路由与认证",
    text: "先检查标准并匹配路由；托管推理在中间件和读取 Body 前认证凭据。",
    flow: [["entry-http"]],
    nodes: ["http-route"],
  },
  {
    order: "3",
    title: "请求中间件",
    text: "认证通过后运行非 WebSocket 请求中间件；拒绝或本地响应可提前结束主链。",
    flow: [["http-middleware"]],
    nodes: ["middleware"],
  },
  {
    order: "4",
    title: "推理准入",
    text: "中间件通过后解析有界 Body，验证模型、Grant、能力、限额与预算。",
    flow: [["middleware-inference"]],
    nodes: ["inference"],
  },
  {
    order: "5",
    title: "模型目标决策",
    text: "resolve_routing 取得并固定本请求的有效目标窗口和路由证据。",
    flow: [["inference-policy"]],
    nodes: ["model-policy"],
  },
  {
    order: "6",
    title: "后端与记账",
    text: "prepare_next 准备目标与 Backend。本例启用耐久用量；记录 request / attempt 成功后才调用上游，失败即拒绝。",
    flow: [["policy-backend"], ["usage-begin"]],
    nodes: ["backend-select", "usage"],
  },
  {
    order: "7",
    title: "上游执行",
    text: "记账开始后，已准备的 attempt 经对应协议处理器发往真实上游。",
    flow: [["backend-proxy"], ["proxy-upstream"]],
    nodes: ["proxy", "upstreams"],
  },
  {
    order: "R",
    title: "前响应回退",
    text: "条件分支：只在响应尚未开始、请求可重放且策略允许时，准备并记录新的 attempt 后重新调用；已经开始的流不切模型。",
    flow: [
      ["retry-policy"],
      ["policy-backend"],
      ["usage-begin"],
      ["backend-proxy"],
      ["proxy-upstream"],
    ],
    nodes: [],
    failed: "upstreams",
  },
  {
    order: "8",
    title: "响应与流",
    text: "响应体返回与用量观察在同一流生命周期内发生；Body 完成或取消后核对并释放许可。不是收到 Headers 就结束。",
    flow: [["upstream-response"], ["response-caller", "response-usage"]],
    nodes: ["response", "usage"],
  },
].map((stage) => ({ ...stage, edges: stage.flow.flat() }));

export class PlaybackClock {
  offset = 0;
  anchor = 0;
  playing = false;
  time(now) {
    return this.offset + (this.playing ? now - this.anchor : 0);
  }
  play(now) {
    if (!this.playing) {
      this.anchor = now;
      this.playing = true;
    }
  }
  pause(now) {
    this.offset = this.time(now);
    this.playing = false;
  }
  seek(time, now) {
    this.offset = Math.max(0, time);
    this.anchor = now;
  }
}
export function frameAt(time) {
  const cycle = STEP_DURATION * SYSTEM_STAGES.length;
  const position = ((time % cycle) + cycle) % cycle;
  const step = Math.floor(position / STEP_DURATION);
  const progress = (position % STEP_DURATION) / STEP_DURATION;
  const stage = SYSTEM_STAGES[step];
  const hopPosition = progress * stage.flow.length;
  return { step, progress, stage, hop: Math.floor(hopPosition), hopProgress: hopPosition % 1 };
}

function svgElement(tag, attributes, parent) {
  const element = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
  parent.append(element);
  return element;
}

function platform(parent, x, y, width, depth, thickness) {
  const top = [
    [x, y - depth],
    [x + width / 2, y],
    [x, y + depth],
    [x - width / 2, y],
  ];
  const points = (vertices) => vertices.map((point) => point.join(",")).join(" ");
  svgElement(
    "polygon",
    {
      points: points([top[3], top[2], [x, y + depth + thickness], [x - width / 2, y + thickness]]),
      fill: "#f0f2ed",
      class: "node-platform",
    },
    parent,
  );
  svgElement(
    "polygon",
    {
      points: points([top[2], top[1], [x + width / 2, y + thickness], [x, y + depth + thickness]]),
      fill: "#f5f6f2",
      class: "node-platform",
    },
    parent,
  );
  svgElement("polygon", { points: points(top), fill: "#fdfefb", class: "node-platform" }, parent);
}

function cube(parent, x, y, size, height) {
  const half = size / 2;
  const depth = size / 4;
  svgElement(
    "polygon",
    {
      points: `${x - half},${y} ${x},${y + depth} ${x},${y + depth - height} ${x - half},${y - height}`,
      fill: "#f1f3ee",
      class: "node-face cube-side",
    },
    parent,
  );
  svgElement(
    "polygon",
    {
      points: `${x},${y + depth} ${x + half},${y} ${x + half},${y - height} ${x},${y + depth - height}`,
      fill: "#f7f8f4",
      class: "node-face cube-side",
    },
    parent,
  );
  return svgElement(
    "polygon",
    {
      points: `${x},${y - depth - height} ${x + half},${y - height} ${x},${y + depth - height} ${x - half},${y - height}`,
      fill: "#fcfdf9",
      class: "node-face cube-top",
    },
    parent,
  );
}

function drawIsoNode(parent, point, kind, compact) {
  const group = svgElement("g", {}, parent);
  const size = compact ? 75 : 92;
  const cubes = [];
  if (kind >= 3) {
    const width = compact ? 110 : 164;
    platform(group, point.x, point.y, width, width / 4, 8);
    for (let row = 0; row < 2; row += 1) {
      for (let column = 0; column < 3; column += 1) {
        const u = (column - 1) * (compact ? 20 : 28);
        const v = (row - 0.5) * (compact ? 22 : 30);
        cubes.push(
          cube(
            group,
            point.x + (u - v) * 0.77,
            point.y + (u + v) * 0.35 - 2,
            compact ? 16 : 23,
            compact ? 11 : 16,
          ),
        );
      }
    }
  } else {
    platform(group, point.x, point.y, size, size / 4, 8);
    if (kind === 0) {
      for (const [dx, dy] of [
        [0, -8],
        [-15, 0],
        [15, 0],
        [0, 8],
      ]) {
        svgElement(
          "ellipse",
          { cx: point.x + dx, cy: point.y + dy, rx: 3.2, ry: 1.8, fill: "#b7bfb3" },
          group,
        );
      }
    } else if (kind === 1) {
      platform(group, point.x, point.y - 8, size * 0.62, size * 0.155, 6);
      svgElement(
        "ellipse",
        {
          cx: point.x,
          cy: point.y - 16,
          rx: size * 0.2,
          ry: size * 0.1,
          fill: "#fdfefb",
          stroke: "#87907f",
          "stroke-width": 1.6,
        },
        group,
      );
      svgElement("circle", { cx: point.x, cy: point.y - 16, r: 2.2, fill: "#58634f" }, group);
    } else {
      cubes.push(cube(group, point.x, point.y - 1, size * 0.42, 22));
      svgElement(
        "path",
        {
          d: `M${point.x - 7},${point.y - 22}l7,-3.5 7,3.5 -7,3.5Z`,
          fill: "none",
          stroke: "#9aa58f",
          "stroke-width": 1,
        },
        group,
      );
    }
  }
  return { group, cubes };
}

function mount() {
  const map = document.querySelector("#system-map");
  const svg = document.querySelector("#architecture-svg");
  const labels = document.querySelector("#node-labels");
  const tooltip = document.querySelector("#node-tooltip");
  const playButton = document.querySelector("#play-toggle");
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  const clock = new PlaybackClock();
  const capabilities = new Map(CAPABILITY_GROUPS.map((group) => [group.id, group]));
  const nodeStages = new Map(
    DIAGRAM_NODES.map(([id]) => [id, SYSTEM_STAGES.filter((stage) => stage.nodes.includes(id))]),
  );
  const uploadStart = SYSTEM_STAGES.findIndex((stage) => stage.order === "7");
  const drawings = new Map();
  const buttons = new Map();
  const bars = [];
  let previousStep = -1;
  let previousHop = -1;
  let hovered;
  let dismissTimer;
  let animationId;
  let requestedPlaying = !reduced.matches;
  svgElement(
    "rect",
    { x: 180, y: 195, width: 1260, height: 740, rx: 16, class: "gateway-boundary" },
    svg,
  );
  for (const [x, title] of [
    [90, "调用方"],
    [270, "监听接入"],
    [450, "路由匹配"],
    [630, "请求策略"],
    [900, "模型目标"],
    [1170, "服务选择"],
    [1350, "协议转发"],
    [1530, "外部服务 / 宿主"],
  ]) {
    svgElement(
      "text",
      { x, y: 225, class: "column-label", "text-anchor": "middle" },
      svg,
    ).textContent = title;
  }
  for (const x of [360, 540, 720, 1080, 1260])
    svgElement("path", { d: `M${x},245V760`, class: "column-divider" }, svg);
  const defs = svgElement("defs", {}, svg);
  const arrow = svgElement(
    "marker",
    {
      id: "flow-arrow",
      markerWidth: 7,
      markerHeight: 7,
      refX: 7,
      refY: 3.5,
      orient: "auto",
      markerUnits: "userSpaceOnUse",
    },
    defs,
  );
  svgElement("path", { d: "M0,0L7,3.5L0,7Z", fill: "context-stroke" }, arrow);
  const edgeLabels = new Map([
    ["entry-http", [355, 475, "HTTP"]],
    ["ordinary-backend", [900, 475, "普通代理"]],
    ["middleware-inference", [760, 425, "托管推理"]],
    ["raw-upstream", [900, 705, "TCP / UDP · 独立中继"]],
    ["response-caller", [660, 889, "HTTP 响应返回"]],
    ["retry-policy", [1170, 200, "R · 条件：响应前回退"]],
    ["direct-http", [1080, 399, "合格普通 HTTP 的直接转发"]],
    ["usage-begin", [1125, 568, "6 · 先记账，再调用"]],
    ["response-usage", [1170, 815, "8 · 随流观察 / 完成核对"]],
    ["usage-cloud", [1260, 959, "异步上报 · 不阻塞响应"]],
  ]);
  const wireLayer = svgElement("g", { class: "wire-layer" }, svg);
  const wires = SYSTEM_EDGES.map((edge) => {
    const group = svgElement(
      "g",
      {
        class: `wire ${edge.kind}${edge.secondary ? " secondary" : ""}${edge.detailOnly ? " detail-only" : ""}`,
        "data-edge": edge.id,
      },
      wireLayer,
    );
    const path = svgElement(
      "path",
      { d: roundedPath(edgeWaypoints(edge)), "marker-end": "url(#flow-arrow)" },
      group,
    );
    const label = edgeLabels.get(edge.id);
    if (label) {
      const [x, y, text] = label;
      svgElement(
        "text",
        { x, y, class: "wire-label", "text-anchor": "middle" },
        group,
      ).textContent = text;
    }
    const packet = svgElement("circle", { r: 3.4 }, group);
    return { edge, group, path, packet, length: path.getTotalLength() };
  });
  const nodeLayer = svgElement("g", { class: "node-layer" }, svg);

  function hideTooltip() {
    clearTimeout(dismissTimer);
    tooltip.hidden = true;
    hovered = undefined;
    buttons.forEach((button) => button.removeAttribute("aria-describedby"));
    drawings.forEach((drawing) => drawing.group.classList.remove("selected"));
    wires.forEach((wire) => wire.group.classList.remove("related"));
  }
  function dismissTooltip() {
    dismissTimer = setTimeout(hideTooltip, 120);
  }
  function positionTooltip() {
    if (tooltip.hidden || !hovered) return;
    const anchor = buttons.get(hovered).getBoundingClientRect();
    const width = tooltip.offsetWidth,
      height = tooltip.offsetHeight;
    const right = anchor.right + 14;
    let left = right + width <= window.innerWidth - 16 ? right : anchor.left - width - 14;
    if (left < 16) left = Math.max(16, window.innerWidth - width - 16);
    tooltip.style.left = `${left}px`;
    tooltip.style.top = `${Math.max(16, Math.min(anchor.top - 40, window.innerHeight - height - 16))}px`;
  }
  function showTooltip(id) {
    clearTimeout(dismissTimer);
    const group = capabilities.get(id);
    if (hovered !== id) {
      document.querySelector("#detail-title").textContent = group.title;
      document.querySelector("#detail-owner").textContent = group.owner;
      document.querySelector("#detail-sequence").textContent =
        id === "cloud-usage"
          ? "异步 · 本地记录可用后后台投递，不作为请求的下一同步步骤"
          : nodeStages
              .get(id)
              .map((stage) => `${stage.order} ${stage.title}`)
              .join(" → ");
      document.querySelector("#detail-description").textContent = group.description;
      document.querySelector("#detail-source").textContent = group.source;
      const list = document.querySelector("#detail-features");
      list.replaceChildren();
      group.features.forEach(([featureId, name, state]) => {
        const item = document.createElement("li");
        item.dataset.feature = featureId;
        item.dataset.state = state;
        const marker = document.createElement("i");
        marker.className = `capability-state ${state}`;
        marker.setAttribute("aria-hidden", "true");
        const label = document.createElement("span");
        label.className = "capability-name";
        label.textContent = name;
        const status = document.createElement("small");
        status.textContent = CAPABILITY_STATES[state];
        item.append(marker, label, status);
        list.append(item);
      });
    }
    hovered = id;
    tooltip.dataset.group = id;
    tooltip.hidden = false;
    buttons.forEach((button, key) =>
      key === id
        ? button.setAttribute("aria-describedby", "node-tooltip")
        : button.removeAttribute("aria-describedby"),
    );
    drawings.forEach((drawing, key) => drawing.group.classList.toggle("selected", key === id));
    wires.forEach(({ edge, group }) =>
      group.classList.toggle("related", edge.from === id || edge.to === id),
    );
    positionTooltip();
  }
  tooltip.addEventListener("pointerenter", () => clearTimeout(dismissTimer));
  tooltip.addEventListener("pointerleave", dismissTooltip);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") hideTooltip();
  });
  new ResizeObserver(positionTooltip).observe(tooltip);

  DIAGRAM_NODES.forEach(([id, x, y, kind]) => {
    const group = capabilities.get(id);
    const drawing = drawIsoNode(nodeLayer, { x, y }, kind, kind >= 3);
    drawing.group.setAttribute(
      "class",
      `isometric-node${id === "entrypoints" ? " gateway-entry" : ""}`,
    );
    drawing.group.dataset.visual = id;
    drawings.set(id, drawing);
    const button = document.createElement("button");
    button.type = "button";
    button.className = `node-label${id === "entrypoints" ? " gateway-entry" : ""}`;
    button.dataset.node = id;
    button.style.left = `${(x / DIAGRAM_SIZE.width) * 100}%`;
    button.style.top = `${(y / DIAGRAM_SIZE.height) * 100}%`;
    button.style.setProperty("--label-gap", kind >= 3 ? "39px" : "32px");
    button.setAttribute("aria-label", `${group.title}，查看真实代码路径`);
    const title = document.createElement("strong");
    title.textContent = group.title;
    const status = document.createElement("small");
    status.className = "node-status";
    status.textContent =
      id === "entrypoints"
        ? "统一数据面接入"
        : id === "model-control"
          ? "Studio 专用分支"
          : id === "agent-cli" || id === "native-cli"
            ? "不经 Gateway 数据面"
            : "";
    button.append(title, status);
    const orders = nodeStages.get(id).map((stage) => stage.order);
    if (orders.length || id === "cloud-usage") {
      const badge = document.createElement("span");
      badge.className = `sequence-badge${id === "cloud-usage" ? " asynchronous" : ""}`;
      badge.textContent = orders.length ? orders.join(" · ") : "异步";
      button.prepend(badge);
    }
    button.addEventListener("pointerenter", () => showTooltip(id));
    button.addEventListener("pointerleave", dismissTooltip);
    button.addEventListener("focus", () => showTooltip(id));
    button.addEventListener("blur", dismissTooltip);
    drawing.group.addEventListener("pointerenter", () => showTooltip(id));
    drawing.group.addEventListener("pointerleave", dismissTooltip);
    labels.append(button);
    buttons.set(id, button);
  });
  const stageButtons = SYSTEM_STAGES.map((stage, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.setAttribute("aria-label", `跳到${stage.title}`);
    button.title = stage.text;
    button.dataset.order = stage.order;
    const label = document.createElement("span");
    label.textContent = `${stage.order} ${stage.title}`;
    const track = document.createElement("span");
    track.className = "timeline-track";
    const bar = document.createElement("i");
    track.append(bar);
    bars.push(bar);
    button.append(track, label);
    button.addEventListener("click", () => {
      clock.seek(index * STEP_DURATION, performance.now());
      requestedPlaying = false;
      syncPlayback();
    });
    document.querySelector("#timeline").append(button);
    return button;
  });
  document.querySelector("#timeline").style.setProperty("--stage-count", SYSTEM_STAGES.length);
  function paint(time) {
    const frame = frameAt(time);
    const activeEdges = frame.stage.flow[frame.hop];
    const uploading = frame.step >= uploadStart;
    const stageChanged = frame.step !== previousStep;
    map.dataset.step = frame.step;
    map.dataset.hop = frame.hop;
    map.dataset.playing = clock.playing;
    if (stageChanged) {
      previousStep = frame.step;
      document.querySelector("#stage-title").textContent = frame.stage.title;
      document.querySelector("#stage-index").textContent =
        frame.stage.order === "R" ? "条件分支 R · 示意" : `时序 ${frame.stage.order} / 8 · 示意`;
      stageButtons.forEach((button, index) =>
        index === frame.step
          ? button.setAttribute("aria-current", "step")
          : button.removeAttribute("aria-current"),
      );
    }
    if (stageChanged || frame.hop !== previousHop) {
      previousHop = frame.hop;
      const completed = new Set([
        ...SYSTEM_STAGES.slice(0, frame.step).flatMap((stage) => stage.edges),
        ...frame.stage.flow.slice(0, frame.hop).flat(),
      ]);
      const activeNodes = new Set();
      wires.forEach(({ edge, group }) => {
        const active = activeEdges.includes(edge.id);
        group.classList.toggle("active", active);
        group.classList.toggle("complete", completed.has(edge.id) && !active);
        group.classList.toggle("async-active", uploading && edge.id === "usage-cloud");
        if (active) [edge.from, edge.to].forEach((id) => activeNodes.add(id));
      });
      drawings.forEach((drawing, id) => {
        drawing.group.classList.toggle("active", activeNodes.has(id));
        drawing.group.classList.toggle("failed", id === frame.stage.failed && frame.hop === 0);
      });
      buttons.forEach((button, id) =>
        button.classList.toggle("current", activeNodes.has(id) && frame.stage.nodes.includes(id)),
      );
    }
    bars.forEach(
      (bar, index) =>
        (bar.style.transform = `scaleX(${index < frame.step ? 1 : index === frame.step ? frame.progress : 0})`),
    );
    wires.forEach(({ edge, path, packet, length }) => {
      const background = uploading && edge.id === "usage-cloud";
      const active = activeEdges.includes(edge.id);
      packet.style.display = active || background ? "" : "none";
      if (!active && !background) return;
      const progress = background ? (time / STEP_DURATION) % 1 : frame.hopProgress;
      const point = path.getPointAtLength(progress * length);
      packet.setAttribute("cx", point.x);
      packet.setAttribute("cy", point.y);
    });
    drawings.forEach((drawing) =>
      drawing.cubes.forEach((cube, i) => {
        cube.style.fill = drawing.group.classList.contains("failed")
          ? "#ead4d3"
          : drawing.group.classList.contains("active") &&
              i === Math.floor(time / 450) % drawing.cubes.length
            ? "#acb6a7"
            : "";
      }),
    );
  }
  function tick(now) {
    paint(clock.time(now));
    if (clock.playing) animationId = requestAnimationFrame(tick);
  }
  function syncPlayback() {
    cancelAnimationFrame(animationId);
    if (requestedPlaying && !document.hidden) clock.play(performance.now());
    else clock.pause(performance.now());
    playButton.textContent = clock.playing ? "Ⅱ" : "▶";
    playButton.setAttribute("aria-label", clock.playing ? "暂停动画" : "播放动画");
    paint(clock.time(performance.now()));
    if (clock.playing) animationId = requestAnimationFrame(tick);
  }
  playButton.addEventListener("click", () => {
    requestedPlaying = !requestedPlaying;
    syncPlayback();
  });
  document.querySelector("#replay").addEventListener("click", () => {
    clock.seek(0, performance.now());
    requestedPlaying = !reduced.matches;
    syncPlayback();
  });
  document.addEventListener("visibilitychange", syncPlayback);
  reduced.addEventListener("change", () => {
    if (reduced.matches) {
      requestedPlaying = false;
      syncPlayback();
    }
  });
  function fitCanvas() {
    hideTooltip();
    const scale = Math.min(
      (window.innerWidth - 48) / DIAGRAM_SIZE.width,
      Math.max(500, window.innerHeight - 145) / DIAGRAM_SIZE.height,
      1.15,
    );
    const stage = document.querySelector("#diagram-stage");
    stage.style.width = `${DIAGRAM_SIZE.width * scale}px`;
    stage.style.height = `${DIAGRAM_SIZE.height * scale}px`;
    document.querySelector("#diagram-canvas").style.setProperty("--scale", scale);
  }
  window.addEventListener("resize", fitCanvas);
  window.addEventListener("scroll", hideTooltip, { passive: true });
  fitCanvas();
  syncPlayback();
}
if (typeof document !== "undefined") mount();
