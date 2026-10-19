/**
 * Project a finished display document into OpenUI form source.
 *
 * The model fence stays data. This walk only accepts a document that already
 * declares editable fields under `state.form`, then emits catalog statements.
 * Other JSON stays source.
 */

const LABEL_CAP = 80;
const TEXT_CAP = 500;
const FIELD_CAP = 12;
const NODE_CAP = 24;
const RECORD_CAP = 4;

const PROTOCOL_KEYS = new Set([
  "schema",
  "ok",
  "success",
  "error",
  "status",
  "exit_code",
  "code",
  "permission_grant",
  "wrote_files",
  "parked_on_log",
  "question_id",
  "tool_call_id",
]);

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

type DisplayForm = { title: string; body: Record<string, Json>; labels: Record<string, string> };

const CONTROL_NAME = /^[A-Za-z][A-Za-z0-9_]{0,47}$/;
const RESERVED_NAMES = new Set(["root", "submit", "__proto__", "prototype", "constructor"]);

class Sink {
  lines: string[] = [];
  seq = 0;
  names = new Set<string>();
  /** A field key cannot be submitted under another name. */
  rejected = false;

  alloc(): string {
    const id = `n${this.seq}`;
    this.seq += 1;
    return id;
  }

  /** Answer key. Statement ids stay local; they are never submitted. */
  controlName(key: string): string | null {
    if (!CONTROL_NAME.test(key) || RESERVED_NAMES.has(key) || this.names.has(key)) {
      this.rejected = true;
      return null;
    }
    this.names.add(key);
    return key;
  }

  card(title: string, children: string[]): string {
    const id = this.alloc();
    this.lines.push(`${id} = Card(${quote(truncate(title, LABEL_CAP))}, [${children.join(", ")}])`);
    return id;
  }
}

export function openUiFormFromJson(source: string): string | null {
  const found = findDeclaredForm(source);
  if (!found) return null;
  const sink = new Sink();
  const children = projectObject(sink, found.body, 0, found);
  if (!children?.length || sink.rejected || sink.seq > NODE_CAP) return null;
  if (!sink.lines.some((line) => / = (Input|Textarea|Checkbox|Select|Slider)\(/.test(line))) return null;
  sink.lines.push('submit = Button("submit", "提交")');
  children.push("submit");
  sink.lines.push(`root = Card(${quote(cardTitle(found))}, [${children.join(", ")}])`);
  const roots = sink.lines.filter((line) => line.trimStart().startsWith("root = ")).length;
  return roots === 1 ? sink.lines.join("\n") : null;
}

function findDeclaredForm(source: string): DisplayForm | null {
  if (source.length > 65_536) return null;
  const parsed = parseJson(source.trim());
  return parsed === undefined ? null : declaredForm(parsed);
}

function declaredForm(value: Json): DisplayForm | null {
  if (!isObject(value) || typeof value.root !== "string" || !value.root.trim()) return null;
  const state = value.state;
  if (!isObject(state) || !isObject(state.form) || Object.keys(state.form).length === 0) return null;
  const title = typeof value.title === "string" && value.title.trim() ? value.title.trim() : value.root.trim();
  return {
    title: truncate(title, LABEL_CAP),
    body: state.form,
    labels: stringLabels(state.labels),
  };
}

function parseJson(text: string): Json | undefined {
  try {
    return JSON.parse(text) as Json;
  } catch {
    return undefined;
  }
}

function projectObject(sink: Sink, value: Record<string, Json>, depth: number, form: DisplayForm): string[] | null {
  if (sink.seq >= NODE_CAP) return null;
  const ids: string[] = [];
  const keys = Object.keys(value).slice(0, FIELD_CAP);
  for (const key of keys) {
    if (PROTOCOL_KEYS.has(key)) continue;
    const child = value[key];
    if ((key === "title" || key === "name" || key === "label") && typeof child === "string") continue;
    const id = projectField(sink, key, child, depth, form);
    if (id) ids.push(id);
  }
  return ids.length ? ids : null;
}

function projectField(sink: Sink, key: string, value: Json, depth: number, form: DisplayForm): string | null {
  if (sink.seq >= NODE_CAP) return null;
  const labeled = labeledScalar(value);
  if (labeled) return projectField(sink, key, labeled.value, depth, { ...form, labels: { ...form.labels, [key]: labeled.label } });
  const label = fieldLabel(form, key);
  if (!label.trim()) return null;
  if (typeof value === "string") {
    const text = truncate(value.trim(), TEXT_CAP);
    if (!text) return null;
    const name = sink.controlName(key);
    if (!name) return null;
    const id = sink.alloc();
    if (text.includes("\n") || [...text].length > 80) {
      sink.lines.push(`${id} = Textarea(${quote(name)}, ${quote(label)}, ${quote(text)})`);
    } else {
      sink.lines.push(`${id} = Input(${quote(name)}, ${quote(label)}, ${quote(text)}, "text")`);
    }
    return id;
  }
  if (typeof value === "number") {
    const number = formatNumber(value);
    if (!number) return null;
    const name = sink.controlName(key);
    if (!name) return null;
    const id = sink.alloc();
    sink.lines.push(`${id} = Input(${quote(name)}, ${quote(label)}, ${number}, "number")`);
    return id;
  }
  if (typeof value === "boolean") {
    const name = sink.controlName(key);
    if (!name) return null;
    const id = sink.alloc();
    sink.lines.push(`${id} = Checkbox(${quote(name)}, ${quote(label)}, ${value})`);
    return id;
  }
  if (isObject(value) && depth < 2) {
    const slider = sliderField(sink, key, label, value);
    if (slider) return slider;
    const select = selectField(sink, key, label, value);
    if (select) return select;
    const children = projectObject(sink, value, depth + 1, form);
    if (!children?.length) return null;
    return sink.card(label, children);
  }
  if (Array.isArray(value) && depth < 2 && value.length >= 2 && value.every(isObject)) {
    const ids: string[] = [];
    for (const item of value.slice(0, RECORD_CAP)) {
      if (!isObject(item)) continue;
      const title = titleOf(item) ?? String(ids.length + 1);
      const children = projectObject(sink, item, depth + 1, form);
      if (!children?.length) continue;
      ids.push(sink.card(title, children));
    }
    return ids.length ? sink.card(label, ids) : null;
  }
  return null;
}

function sliderField(sink: Sink, key: string, label: string, value: Record<string, Json>): string | null {
  const initial = value.value ?? value.initial;
  const min = value.min;
  const max = value.max;
  if (typeof initial !== "number" || typeof min !== "number" || typeof max !== "number") return null;
  if (!Number.isFinite(initial) || !Number.isFinite(min) || !Number.isFinite(max)) return null;
  if (min >= max || initial < min || initial > max) return null;
  const initialText = formatNumber(initial);
  const minText = formatNumber(min);
  const maxText = formatNumber(max);
  if (!initialText || !minText || !maxText) return null;
  const step = typeof value.step === "number" ? formatNumber(value.step) : null;
  const name = sink.controlName(key);
  if (!name) return null;
  const id = sink.alloc();
  const stepArg = step ? `, ${step}` : "";
  sink.lines.push(`${id} = Slider(${quote(name)}, ${quote(label)}, ${initialText}, ${minText}, ${maxText}${stepArg})`);
  return id;
}

function selectField(sink: Sink, key: string, label: string, value: Record<string, Json>): string | null {
  const selectedRaw = value.value ?? value.initial;
  if (typeof selectedRaw !== "string") return null;
  const selected = truncate(selectedRaw.trim(), LABEL_CAP);
  if (!Array.isArray(value.options) || value.options.length === 0 || value.options.length > 8) return null;
  const options: string[] = [];
  for (const option of value.options) {
    if (typeof option !== "string") return null;
    const text = truncate(option.trim(), LABEL_CAP);
    if (!text || options.includes(text)) return null;
    options.push(text);
  }
  if (!options.includes(selected)) return null;
  const name = sink.controlName(key);
  if (!name) return null;
  const id = sink.alloc();
  sink.lines.push(
    `${id} = Select(${quote(name)}, ${quote(label)}, ${quote(selected)}, [${options.map(quote).join(", ")}])`,
  );
  return id;
}

function cardTitle(form: DisplayForm): string {
  return truncate(form.title, LABEL_CAP);
}

function fieldLabel(form: DisplayForm, key: string): string {
  const given = form.labels[key];
  if (given) return truncate(given, LABEL_CAP);
  return truncate(key, LABEL_CAP);
}

function stringLabels(value: Json | undefined): Record<string, string> {
  if (!isObject(value)) return {};
  const labels: Record<string, string> = {};
  for (const [key, label] of Object.entries(value)) {
    if (typeof label === "string" && label.trim()) labels[key] = label.trim();
  }
  return labels;
}

function labeledScalar(value: Json): { label: string; value: Json } | null {
  if (!isObject(value) || "options" in value || "min" in value || "max" in value) return null;
  if (typeof value.label !== "string" || !value.label.trim() || !("value" in value)) return null;
  const inner = value.value;
  if (typeof inner === "string" || typeof inner === "number" || typeof inner === "boolean") {
    return { label: value.label.trim(), value: inner };
  }
  return null;
}

function titleOf(value: Record<string, Json>): string | null {
  for (const key of ["title", "name", "label"]) {
    const text = value[key];
    if (typeof text === "string" && text.trim()) return truncate(text.trim(), LABEL_CAP);
  }
  return null;
}

function isObject(value: Json | unknown): value is Record<string, Json> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function formatNumber(value: number): string | null {
  if (!Number.isFinite(value)) return null;
  if (Number.isInteger(value)) return String(value);
  const text = value.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
  if (!text || text.length > 24) return null;
  return text;
}

function quote(value: string): string {
  let out = '"';
  for (const char of value) {
    if (char === "\\") out += "\\\\";
    else if (char === '"') out += '\\"';
    else if (char === "\n" || char === "\r") out += " ";
    else if (char.charCodeAt(0) < 32) continue;
    else out += char;
  }
  return `${out}"`;
}

function truncate(value: string, max: number): string {
  return [...value].slice(0, max).join("");
}
