import type { Expression } from "estree";

import { componentCatalog } from "./catalog";

export type Value = string | number | boolean | null | Value[] | { [key: string]: Value };
export type Values = Record<string, string | number | boolean>;
export interface UiNode {
  type: "component" | "text";
  name?: string;
  value?: string;
  children?: UiNode[];
  props?: Record<string, Expression>;
}
export type Document = { nodes: UiNode[]; initial: Values; pending: boolean };

const controls = new Set(["Input", "Textarea", "Slider", "Select", "Checkbox"]);
export const holdsControl = (node: UiNode): boolean =>
  node.type === "component" &&
  (controls.has(node.name ?? "") || (node.children ?? []).some(holdsControl));
const forbidden = new Set(["__proto__", "prototype", "constructor"]);
const fail = (): never => {
  throw new Error("Unsupported OpenUI document");
};

/** Interpret a small expression language; generated JavaScript is never compiled or evaluated. */
export function evaluate(node: Expression, values: Values, depth = 0): Value {
  if (depth > 24) return fail();
  const next = (value: Expression) => evaluate(value, values, depth + 1);
  switch (node.type) {
    case "Literal":
      if (node.value === null || typeof node.value === "string" || typeof node.value === "boolean")
        return node.value;
      if (typeof node.value === "number" && Number.isFinite(node.value)) return node.value;
      return fail();
    case "Identifier":
      if (forbidden.has(node.name) || !Object.hasOwn(values, node.name)) return fail();
      return values[node.name];
    case "ArrayExpression":
      if (node.elements.length > 64) return fail();
      return node.elements.map((element) =>
        !element || element.type === "SpreadElement" ? fail() : next(element),
      );
    case "ObjectExpression": {
      if (node.properties.length > 64) return fail();
      const result: Record<string, Value> = Object.create(null);
      for (const prop of node.properties) {
        if (
          prop.type !== "Property" ||
          prop.computed ||
          prop.method ||
          prop.kind !== "init" ||
          prop.value.type === "AssignmentPattern"
        )
          return fail();
        const key =
          prop.key.type === "Identifier"
            ? prop.key.name
            : prop.key.type === "Literal"
              ? String(prop.key.value)
              : fail();
        if (forbidden.has(key) || Object.hasOwn(result, key)) return fail();
        result[key] = next(prop.value as Expression);
      }
      return result;
    }
    case "UnaryExpression": {
      const value = next(node.argument);
      if (node.operator === "!") return !value;
      if (typeof value !== "number") return fail();
      if (node.operator === "-") return -value;
      if (node.operator === "+") return value;
      return fail();
    }
    case "BinaryExpression": {
      const left = next(node.left as Expression),
        right = next(node.right);
      if (node.operator === "===") return left === right;
      if (node.operator === "!==") return left !== right;
      if (typeof left !== "number" || typeof right !== "number") return fail();
      let result: Value;
      switch (node.operator) {
        case "+":
          result = left + right;
          break;
        case "-":
          result = left - right;
          break;
        case "*":
          result = left * right;
          break;
        case "/":
          result = left / right;
          break;
        case "%":
          result = left % right;
          break;
        case "**":
          result = left ** right;
          break;
        case "<":
          return left < right;
        case ">":
          return left > right;
        case "<=":
          return left <= right;
        case ">=":
          return left >= right;
        default:
          return fail();
      }
      return Number.isFinite(result) ? result : fail();
    }
    case "LogicalExpression": {
      const left = next(node.left),
        right = next(node.right);
      if (node.operator === "&&") return left ? right : left;
      if (node.operator === "||") return left ? left : right;
      if (node.operator === "??") return left ?? right;
      return fail();
    }
    case "ConditionalExpression": {
      // Validate both branches, including ones not currently visible.
      const test = next(node.test),
        yes = next(node.consequent),
        no = next(node.alternate);
      return test ? yes : no;
    }
    default:
      return fail();
  }
}

export function propsOf(node: UiNode, values: Values): Record<string, Value> {
  const result: Record<string, Value> = Object.create(null);
  for (const [name, expression] of Object.entries(node.props ?? {})) {
    result[name] = evaluate(expression, values);
  }
  return result;
}

export function validate(nodes: UiNode[]): Values {
  const initial: Values = Object.create(null);
  const flat: UiNode[] = [];
  function walk(node: UiNode, depth: number) {
    if (depth > 20 || flat.length >= 160) return fail();
    flat.push(node);
    if (node.type === "component") {
      if (!node.name || !Object.hasOwn(componentCatalog, node.name)) return fail();
      for (const name of Object.keys(node.props ?? {})) {
        if (!componentCatalog[node.name].props.includes(name)) return fail();
      }
      if (controls.has(node.name)) {
        // Control definitions must be literals, independent of other controls.
        const p = propsOf(node, {});
        if (
          typeof p.name !== "string" ||
          !/^[A-Za-z][A-Za-z0-9_]{0,47}$/.test(p.name) ||
          forbidden.has(p.name) ||
          Object.hasOwn(initial, p.name)
        )
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
        if (typeof value === "string" && value.length > 4096) return fail();
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
    const p = propsOf(node, initial);
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
        if (p[key] !== undefined && typeof p[key] !== "number") return fail();
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
        p.labels.some((v) => typeof v !== "string") ||
        p.values.some((v) => typeof v !== "number" || v < 0))
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
      const title = propsOf(node, {}).title;
      if (typeof title === "string") result.title = title;
    }
    if (node.name && controls.has(node.name)) {
      const p = propsOf(node, {});
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
      const p = propsOf(node, {});
      const value = values[p.name as string];
      const expected =
        node.name === "Checkbox"
          ? "boolean"
          : node.name === "Slider" || p.type === "number"
            ? "number"
            : "string";
      if (typeof value !== expected) return false;
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
          (typeof p.min === "number" && value < p.min) ||
          (typeof p.max === "number" && value > p.max))
      )
        return false;
    }
    if (node.children && !validAnswers(node.children, values)) return false;
  }
  return true;
}
