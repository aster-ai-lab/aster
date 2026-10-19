/**
 * Project an agent JSON Schema onto the closed OpenUI catalog.
 * The accepted subset matches `agent_parameters.rs`.
 */

import { compileOpenUi } from "./openui";

export type AgentParameterKind = "string" | "number" | "integer" | "boolean";

export type AgentParameterField = {
  name: string;
  label: string;
  kind: AgentParameterKind;
  required: boolean;
  minimum?: number;
  maximum?: number;
  maxLength?: number;
  enumValues?: Array<string | number>;
  longText: boolean;
  slider: boolean;
};

export type ParameterValues = Record<string, string | number | boolean>;

const NAME = /^[A-Za-z][A-Za-z0-9_]{0,47}$/;
const RESERVED = new Set(["root", "submit", "__proto__", "prototype", "constructor"]);
const PROPERTY_KEYS = new Set(["type", "title", "enum", "minimum", "maximum", "maxLength"]);

export function projectAgentParameterSchema(
  source: string,
): { ok: true; fields: AgentParameterField[]; openUi: string } | { ok: false; error: "invalid" | "limit" } {
  if (source.length === 0 || source.length > 65_536) return { ok: false, error: "limit" };
  let parsed: unknown;
  try {
    parsed = JSON.parse(source);
  } catch {
    return { ok: false, error: "invalid" };
  }
  const fields = schemaFields(parsed);
  if (!fields) return { ok: false, error: "invalid" };
  const openUi = openUiFor(fields);
  if (!openUi || !compileOpenUi(openUi, false).ok) return { ok: false, error: "invalid" };
  return { ok: true, fields, openUi };
}

export function parameterValuesReady(fields: readonly AgentParameterField[], values: ParameterValues): boolean {
  return validateParameterValues(fields, values).ok;
}

export function validateParameterValues(
  fields: readonly AgentParameterField[],
  values: ParameterValues,
): { ok: true; values: ParameterValues } | { ok: false } {
  if (Object.keys(values).length > 12) return { ok: false };
  const names = new Set(fields.map((field) => field.name));
  for (const key of Object.keys(values)) {
    if (!names.has(key)) return { ok: false };
  }
  const checked: ParameterValues = {};
  for (const field of fields) {
    if (!Object.hasOwn(values, field.name)) {
      if (field.required) return { ok: false };
      continue;
    }
    const value = checkedValue(field, values[field.name]);
    if (value === undefined) return { ok: false };
    checked[field.name] = value;
  }
  return { ok: true, values: checked };
}

/** Keep extracted keys that still match the schema. A bad extract fills nothing. */
export function acceptedParameterPrefill(fields: readonly AgentParameterField[], raw: unknown): ParameterValues {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const candidate: ParameterValues = {};
  for (const field of fields) {
    const value = (raw as Record<string, unknown>)[field.name];
    if (typeof value !== "string" && typeof value !== "number" && typeof value !== "boolean") continue;
    const checked = checkedValue(field, value);
    if (checked !== undefined) candidate[field.name] = checked;
  }
  return candidate;
}

/**
 * Fill fields the user has not edited and has not already given a value.
 * A late extract uses the generation the caller captured when the mention started.
 */
export function mergeParameterPrefill(
  current: ParameterValues,
  extracted: ParameterValues,
  edited: ReadonlySet<string>,
): ParameterValues {
  const next = { ...current };
  for (const [key, value] of Object.entries(extracted)) {
    if (edited.has(key) || Object.hasOwn(current, key)) continue;
    next[key] = value;
  }
  return next;
}

function schemaFields(value: unknown): AgentParameterField[] | null {
  if (!isObject(value)) return null;
  if (value.type !== "object") return null;
  for (const key of Object.keys(value)) {
    if (key !== "type" && key !== "properties" && key !== "required") return null;
  }
  if (!isObject(value.properties)) return null;
  const entries = Object.entries(value.properties);
  if (entries.length < 1 || entries.length > 12) return null;
  const required = new Set<string>();
  if (value.required !== undefined) {
    if (!Array.isArray(value.required)) return null;
    for (const item of value.required) {
      if (typeof item !== "string" || required.has(item) || !Object.hasOwn(value.properties, item)) return null;
      required.add(item);
    }
  }
  const fields: AgentParameterField[] = [];
  for (const [name, spec] of entries) {
    if (!NAME.test(name) || RESERVED.has(name) || !isObject(spec)) return null;
    const field = readField(name, spec, required.has(name));
    if (!field) return null;
    fields.push(field);
  }
  return fields;
}

function readField(name: string, spec: Record<string, unknown>, required: boolean): AgentParameterField | null {
  for (const key of Object.keys(spec)) {
    if (!PROPERTY_KEYS.has(key)) return null;
  }
  const kind = spec.type;
  if (kind !== "string" && kind !== "number" && kind !== "integer" && kind !== "boolean") return null;
  let label = name;
  if (spec.title !== undefined) {
    if (typeof spec.title !== "string" || spec.title.trim().length === 0 || [...spec.title].length > 80) return null;
    label = spec.title.trim();
  }
  let maxLength: number | undefined;
  if (spec.maxLength !== undefined) {
    if (kind !== "string" || !whole(spec.maxLength) || spec.maxLength < 1 || spec.maxLength > 4096) return null;
    maxLength = spec.maxLength;
  }
  const minimum = bound(spec.minimum, kind);
  const maximum = bound(spec.maximum, kind);
  if (spec.minimum !== undefined && minimum === undefined) return null;
  if (spec.maximum !== undefined && maximum === undefined) return null;
  if (minimum !== undefined && maximum !== undefined && minimum > maximum) return null;
  const enumValues = readEnum(spec.enum, kind);
  if (spec.enum !== undefined && !enumValues) return null;
  const longText = kind === "string" && (maxLength ?? 0) > 80 && !enumValues;
  const slider =
    kind === "integer" &&
    minimum !== undefined &&
    maximum !== undefined &&
    maximum - minimum <= 100 &&
    !enumValues;
  return {
    name,
    label,
    kind,
    required,
    ...(minimum !== undefined ? { minimum } : {}),
    ...(maximum !== undefined ? { maximum } : {}),
    ...(maxLength !== undefined ? { maxLength } : {}),
    ...(enumValues ? { enumValues } : {}),
    longText,
    slider,
  };
}

function bound(value: unknown, kind: AgentParameterKind): number | undefined {
  if (value === undefined) return undefined;
  if (kind !== "number" && kind !== "integer") return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  if (kind === "integer" && !Number.isInteger(value)) return undefined;
  return value;
}

function readEnum(value: unknown, kind: AgentParameterKind): Array<string | number> | null {
  if (value === undefined) return null;
  if (kind === "boolean" || !Array.isArray(value) || value.length < 1 || value.length > 8) return null;
  const values: Array<string | number> = [];
  for (const item of value) {
    if (kind === "string") {
      if (typeof item !== "string" || item.length === 0 || item.length > 80) return null;
      if (values.includes(item)) return null;
      values.push(item);
    } else {
      if (typeof item !== "number" || !Number.isFinite(item)) return null;
      if (kind === "integer" && !Number.isInteger(item)) return null;
      if (values.includes(item)) return null;
      values.push(item);
    }
  }
  return values;
}

function checkedValue(field: AgentParameterField, value: string | number | boolean | undefined): string | number | boolean | undefined {
  if (field.enumValues && !field.enumValues.some((item) => item === value)) return undefined;
  if (field.kind === "string") {
    if (typeof value !== "string" || value.includes("\u0000")) return undefined;
    if (value.length > (field.maxLength ?? 4096)) return undefined;
    if (field.required && value.trim().length === 0) return undefined;
    return value;
  }
  if (field.kind === "boolean") return typeof value === "boolean" ? value : undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  if (field.kind === "integer" && !Number.isInteger(value)) return undefined;
  if (field.minimum !== undefined && value < field.minimum) return undefined;
  if (field.maximum !== undefined && value > field.maximum) return undefined;
  return value;
}

function openUiFor(fields: readonly AgentParameterField[]): string | null {
  const lines: string[] = [];
  const children: string[] = [];
  fields.forEach((field, index) => {
    const id = `n${index}`;
    const line = fieldLine(id, field);
    if (!line) return;
    lines.push(line);
    children.push(id);
  });
  if (children.length !== fields.length) return null;
  lines.push('submit = Button("submit", "提交")');
  children.push("submit");
  lines.push(`root = Card("参数", [${children.join(", ")}])`);
  return lines.join("\n");
}

function fieldLine(id: string, field: AgentParameterField): string | null {
  const name = quote(field.name);
  const label = quote(field.label);
  if (field.enumValues && field.enumValues.every((item) => typeof item === "string")) {
    const selected = quote(String(field.enumValues[0]));
    const options = field.enumValues.map((item) => quote(String(item))).join(", ");
    return `${id} = Select(${name}, ${label}, ${selected}, [${options}])`;
  }
  if (field.slider && field.minimum !== undefined && field.maximum !== undefined) {
    return `${id} = Slider(${name}, ${label}, ${field.minimum}, ${field.minimum}, ${field.maximum}, 1)`;
  }
  if (field.kind === "boolean") return `${id} = Checkbox(${name}, ${label}, false)`;
  if (field.longText) return `${id} = Textarea(${name}, ${label}, "")`;
  if (field.kind === "string") return `${id} = Input(${name}, ${label}, "", "text")`;
  return `${id} = Input(${name}, ${label}, 0, "number")`;
}

function quote(value: string): string {
  return JSON.stringify(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function whole(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value);
}
