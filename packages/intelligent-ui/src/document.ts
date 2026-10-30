import { componentCatalog } from "./catalog";

export type Value = string | number | boolean | null | Value[] | { [key: string]: Value };
export type Values = Record<string, string | number | boolean>;
export interface UiNode {
  type: "component" | "text";
  name?: string;
  value?: string;
  children?: UiNode[];
  props?: Record<string, Value>;
}
export type Document = { nodes: UiNode[]; initial: Values; pending: boolean };

const controls = new Set(["Input", "Textarea", "Slider", "Select", "Checkbox"]);
export const holdsControl = (node: UiNode): boolean =>
  node.type === "component" &&
  (controls.has(node.name ?? "") || (node.children ?? []).some(holdsControl));
export const identifier = (name: string) =>
  /^[A-Za-z][A-Za-z0-9_]{0,47}$/.test(name) &&
  !["__proto__", "prototype", "constructor"].includes(name);
export const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const fail = (): never => {
  throw new Error("Unsupported JSON Render document");
};
export function validate(nodes: UiNode[]): Values {
  const initial: Values = Object.create(null);
  const flat: UiNode[] = [];
  function walk(node: UiNode, depth: number) {
    if (depth > 20 || flat.length >= 160) return fail();
    flat.push(node);
    if (node.type === "component") {
      if (!node.name || !Object.hasOwn(componentCatalog, node.name)) return fail();
      if (controls.has(node.name)) {
        // Control definitions must be literals, independent of other controls.
        const p = node.props ?? {};
        if (typeof p.name !== "string" || !identifier(p.name) || Object.hasOwn(initial, p.name))
          return fail();
        if (typeof p.label !== "string" || !p.label.trim()) return fail();
        if (p.required !== undefined && typeof p.required !== "boolean") return fail();
        if (p.placeholder !== undefined && typeof p.placeholder !== "string") return fail();
        const value =
          p.initial ??
          (node.name === "Checkbox"
            ? false
            : node.name === "Slider" || p.type === "number"
              ? 0
              : "");
        if (!["string", "number", "boolean"].includes(typeof value)) return fail();
        if (
          typeof value === "number" &&
          (!Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER)
        )
          return fail();
        if (typeof value === "string" && new TextEncoder().encode(value).byteLength > 4096)
          return fail();
        if (node.name === "Checkbox" && typeof value !== "boolean") return fail();
        if ((node.name === "Slider" || p.type === "number") && typeof value !== "number")
          return fail();
        if (
          node.name !== "Checkbox" &&
          node.name !== "Slider" &&
          p.type !== "number" &&
          typeof value !== "string"
        )
          return fail();
        initial[p.name] = value as string | number | boolean;
        if (Object.keys(initial).length > 64) return fail();
      }
    } else if (node.type !== "text") return fail();
    for (const child of node.children ?? []) walk(child, depth + 1);
  }
  for (const node of nodes) walk(node, 0);
  for (const node of flat) {
    if (!node.name) continue;
    const p = node.props ?? {};
    if (node.name === "Form" && p.submitAction !== undefined && p.submitAction !== "submit")
      return fail();
    if (node.name === "Card" && p.title !== undefined && typeof p.title !== "string") return fail();
    if (node.name === "Text" && typeof p.content !== "string") return fail();
    if (
      node.name === "Metric" &&
      (typeof p.label !== "string" ||
        !["string", "number"].includes(typeof p.value) ||
        (typeof p.value === "number" && !Number.isFinite(p.value)) ||
        (p.unit !== undefined && typeof p.unit !== "string"))
    )
      return fail();
    if (
      node.name === "Stack" &&
      ((p.direction !== undefined && !["vertical", "horizontal"].includes(p.direction as string)) ||
        (p.gap !== undefined && (typeof p.gap !== "number" || p.gap < 0 || p.gap > 24)))
    )
      return fail();
    if (node.name === "Button" && (p.action !== "submit" || typeof p.label !== "string"))
      return fail();
    if (node.name === "Grid" && p.columns !== undefined && ![1, 2, 3].includes(p.columns as number))
      return fail();
    if (
      node.name === "Input" &&
      p.type !== undefined &&
      !["text", "number", "email", "tel"].includes(p.type as string)
    )
      return fail();
    if (node.name === "Slider" || node.name === "Input") {
      for (const key of ["min", "max", "step"])
        if (p[key] !== undefined && (typeof p[key] !== "number" || !Number.isFinite(p[key])))
          return fail();
      // min, max, and step constrain sliders and number inputs. Other input types only carry those slots.
      const numeric = node.name === "Slider" || p.type === "number";
      if (numeric) {
        if (p.step !== undefined && (p.step as number) <= 0) return fail();
        if (p.min !== undefined && p.max !== undefined && (p.min as number) >= (p.max as number))
          return fail();
        if (
          node.name === "Slider" &&
          (p.min === undefined || p.max === undefined || p.initial === undefined)
        )
          return fail();
        if (
          typeof p.initial === "number" &&
          ((typeof p.min === "number" && p.initial < p.min) ||
            (typeof p.max === "number" && p.initial > p.max))
        )
          return fail();
      }
    }
    if (
      node.name === "Select" &&
      (!Array.isArray(p.options) ||
        !p.options.length ||
        p.options.length > 64 ||
        p.options.some((v) => typeof v !== "string" || !v) ||
        new Set(p.options).size !== p.options.length ||
        (initial[p.name as string] !== "" && !p.options.includes(initial[p.name as string])))
    )
      return fail();
    if (
      node.name === "BarChart" &&
      (!Array.isArray(p.labels) ||
        !Array.isArray(p.values) ||
        p.labels.length !== p.values.length ||
        p.labels.length > 64 ||
        p.labels.some((v) => typeof v !== "string") ||
        p.values.some((v) => typeof v !== "number" || !Number.isFinite(v) || v < 0))
    )
      return fail();
  }
  return initial;
}

export type FormField = { name: string; label: string; kind: string };
export type FormOutline = { title: string; fields: FormField[] };

/** Title of the first card and every control in source order, for a read-only answer summary. */
export function outline(nodes: UiNode[]): FormOutline {
  const result: FormOutline = { title: "", fields: [] };
  const walk = (node: UiNode) => {
    if (node.name === "Card" && !result.title) {
      const title = (node.props ?? {}).title;
      if (typeof title === "string") result.title = title;
    }
    if (node.name && controls.has(node.name)) {
      const p = node.props ?? {};
      result.fields.push({ name: String(p.name), label: String(p.label), kind: node.name });
    }
    for (const child of node.children ?? []) walk(child);
  };
  for (const node of nodes) walk(node);
  return result;
}

/** Validate the complete answers before sending them to a pending turn. */
export function validAnswers(nodes: UiNode[], values: Values): boolean {
  for (const node of nodes) {
    if (node.name && controls.has(node.name)) {
      const p = node.props ?? {};
      const value = values[p.name as string];
      const expected =
        node.name === "Checkbox"
          ? "boolean"
          : node.name === "Slider" || p.type === "number"
            ? "number"
            : "string";
      if (typeof value !== expected) return false;
      if (typeof value === "string" && new TextEncoder().encode(value).byteLength > 4096)
        return false;
      if (p.required && (value === false || (typeof value === "string" && !value.trim())))
        return false;
      if (
        node.name === "Select" &&
        value !== "" &&
        !(p.options as string[]).includes(value as string)
      )
        return false;
      if (
        typeof value === "number" &&
        (!Number.isFinite(value) ||
          Math.abs(value) > Number.MAX_SAFE_INTEGER ||
          (typeof p.min === "number" && value < p.min) ||
          (typeof p.max === "number" && value > p.max))
      )
        return false;
    }
    if (node.children && !validAnswers(node.children, values)) return false;
  }
  return true;
}
