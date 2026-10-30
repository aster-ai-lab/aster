import { componentCatalog } from "./catalog";
import {
  identifier,
  isRecord,
  outline,
  validate,
  type Document,
  type FormOutline,
  type UiNode,
  type Value,
} from "./document";

export type { FormField, FormOutline } from "./document";
export { jsonRenderFromJson } from "./displayForm";
export {
  acceptedParameterPrefill,
  mergeParameterPrefill,
  parameterValuesReady,
  projectAgentParameterSchema,
  validateParameterValues,
} from "./parameterForm";
export type { AgentParameterField, AgentParameterKind, ParameterValues } from "./parameterForm";

export type JsonRenderResult =
  | { ok: true; document: Document; pending: boolean }
  | { ok: false; error: "invalid" | "limit" };

/** Interpret JSON data directly, with no generated code or expression evaluator. */
export function compileJsonRender(source: string, streaming: boolean): JsonRenderResult {
  if (new TextEncoder().encode(source).byteLength > 65536) return { ok: false, error: "limit" };
  let raw: unknown;
  try {
    raw = JSON.parse(source);
  } catch {
    return streaming
      ? { ok: true, document: { nodes: [], initial: {}, pending: true }, pending: true }
      : { ok: false, error: "invalid" };
  }
  try {
    if (
      !isRecord(raw) ||
      Object.keys(raw).some((key) => !["root", "elements", "state"].includes(key)) ||
      typeof raw.root !== "string" ||
      !isRecord(raw.elements)
    )
      throw 0;
    const elements = raw.elements;
    const ids = Object.keys(elements);
    if (!ids.length || ids.length > 160 || ids.some((id) => !identifier(id))) throw 0;
    const state = raw.state === undefined ? {} : raw.state;
    if (!isRecord(state) || Object.keys(state).some((key) => key !== "form")) throw 0;
    const seeds = state.form === undefined ? {} : state.form;
    if (!isRecord(seeds) || Object.keys(seeds).length > 64) throw 0;
    const seen = new Set<string>();
    const names = new Set<string>();
    const build = (id: string, depth: number): UiNode => {
      if (depth > 20 || seen.has(id) || !Object.hasOwn(elements, id)) throw 0;
      seen.add(id);
      const node = elements[id];
      if (
        !isRecord(node) ||
        Object.keys(node).some((key) => !["type", "props", "children"].includes(key)) ||
        typeof node.type !== "string" ||
        !Object.hasOwn(componentCatalog, node.type) ||
        !isRecord(node.props)
      )
        throw 0;
      const catalog = componentCatalog[node.type];
      if (Object.keys(node.props).some((key) => !catalog.props.includes(key))) throw 0;
      const children = node.children === undefined ? [] : node.children;
      if (
        !Array.isArray(children) ||
        children.length > 64 ||
        children.some((child) => typeof child !== "string") ||
        (!catalog.children && children.length)
      )
        throw 0;
      const p = { ...node.props } as Record<string, Value>;
      if (["Input", "Textarea", "Select", "Slider", "Checkbox"].includes(node.type)) {
        const slot = node.type === "Checkbox" ? "checked" : "value";
        const bind = p[slot];
        if (
          !isRecord(bind) ||
          Object.keys(bind).length !== 1 ||
          typeof bind.$bindState !== "string" ||
          !bind.$bindState.startsWith("/form/")
        )
          throw 0;
        const name = bind.$bindState.slice(6);
        if (!identifier(name) || names.has(name)) throw 0;
        names.add(name);
        if (
          Object.hasOwn(seeds, name) &&
          !["string", "number", "boolean"].includes(typeof seeds[name])
        )
          throw 0;
        delete p[slot];
        p.name = name;
        p.initial = Object.hasOwn(seeds, name)
          ? (seeds[name] as Value)
          : node.type === "Checkbox"
            ? false
            : node.type === "Slider" || p.type === "number"
              ? 0
              : "";
      }
      return {
        type: "component",
        name: node.type,
        props: p,
        children: children.map((child) => build(child as string, depth + 1)),
      };
    };
    const nodes = [build(raw.root, 0)];
    if (seen.size !== ids.length || Object.keys(seeds).some((name) => !names.has(name))) throw 0;
    const initial = validate(nodes);
    return { ok: true, document: { nodes, initial, pending: streaming }, pending: streaming };
  } catch {
    return { ok: false, error: "invalid" };
  }
}

export function describeJsonRenderForm(source: string): FormOutline | null {
  const compiled = compileJsonRender(source, false);
  return compiled.ok ? outline(compiled.document.nodes) : null;
}
