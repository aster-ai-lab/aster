import { parseExpressionAt } from "acorn";
import type { Expression } from "estree";

import { componentCatalog } from "./catalog";
import { outline, validate, type Document, type FormOutline, type UiNode } from "./document";

export type { FormField, FormOutline } from "./document";
export { openUiFormFromJson } from "./jsonForm";
export {
  acceptedParameterPrefill,
  mergeParameterPrefill,
  parameterValuesReady,
  projectAgentParameterSchema,
  validateParameterValues,
} from "./parameterForm";
export type { AgentParameterField, AgentParameterKind, ParameterValues } from "./parameterForm";

export type OpenUiResult =
  | { ok: true; document: Document; pending: boolean }
  | { ok: false; error: "invalid" | "limit" };

type Arg =
  | string
  | number
  | boolean
  | null
  | { expr: string }
  | { refs: string[] }
  | { list: Array<string | number> };

const ident = /^[A-Za-z][A-Za-z0-9_]{0,47}$/;

/** Parse completed OpenUI statements directly into the bounded component document. */
export function compileOpenUi(source: string, streaming: boolean): OpenUiResult {
  if (source.length > 65536) return { ok: false, error: "limit" };
  const empty = (): OpenUiResult => ({
    ok: true,
    document: { nodes: [], initial: {}, pending: streaming },
    pending: streaming,
  });
  const lines = source.split(/\r?\n/).filter((line) => line.trim().length > 0);
  if (streaming && lines.length > 0 && !isStatement(lines[lines.length - 1])) lines.pop();
  if (lines.length > 160) return { ok: false, error: "limit" };
  const nodes = new Map<string, { type: string; args: Arg[] }>();
  for (const line of lines) {
    const statement = parseStatement(line);
    if (!statement || !Object.hasOwn(componentCatalog, statement.type) || nodes.has(statement.name))
      return { ok: false, error: "invalid" };
    nodes.set(statement.name, { type: statement.type, args: statement.args });
  }
  if (!nodes.has("root")) return streaming ? empty() : { ok: false, error: "invalid" };
  try {
    const root = buildNode("root", nodes, new Set(), streaming, { count: 0 });
    const document = { nodes: [root], initial: validate([root]), pending: streaming };
    return { ok: true, document, pending: streaming };
  } catch {
    return { ok: false, error: "invalid" };
  }
}

/** The card title and control labels of a complete source; null when the source does not compile. */
export function describeOpenUiForm(source: string): FormOutline | null {
  const compiled = compileOpenUi(source, false);
  if (!compiled.ok) return null;
  try {
    return outline(compiled.document.nodes);
  } catch {
    return null;
  }
}

function isStatement(line: string): boolean {
  return parseStatement(line.trim()) !== null;
}

function parseStatement(line: string): { name: string; type: string; args: Arg[] } | null {
  const match = line
    .trim()
    .match(/^([A-Za-z][A-Za-z0-9_]{0,47}) = ([A-Za-z][A-Za-z0-9_]{0,47})\((.*)\)\s*$/);
  if (!match) return null;
  const args = parseArgs(match[3]);
  if (!args) return null;
  return { name: match[1], type: match[2], args };
}

function parseArgs(input: string, depth = 0): Arg[] | null {
  if (depth > 20) return null;
  const args: Arg[] = [];
  let i = 0;
  const skip = () => {
    while (i < input.length && /\s/.test(input[i])) i += 1;
  };
  skip();
  while (i < input.length) {
    const value = parseArg(input, i, depth);
    if (!value) return null;
    args.push(value.value);
    i = value.next;
    skip();
    if (i === input.length) break;
    if (input[i] !== ",") return null;
    i += 1;
    skip();
    if (i === input.length) return null;
  }
  return args;
}

function parseArg(
  input: string,
  start: number,
  depth: number,
): { value: Arg; next: number } | null {
  const char = input[start];
  if (char === '"') {
    let i = start + 1;
    while (i < input.length) {
      if (input[i] === "\\") {
        i += 2;
        continue;
      }
      if (input[i] === '"') {
        try {
          return { value: JSON.parse(input.slice(start, i + 1)), next: i + 1 };
        } catch {
          return null;
        }
      }
      i += 1;
    }
    return null;
  }
  if (char === "{") {
    const end = closingDelimiter(input, start, "}");
    if (end < 0) return null;
    const expr = input.slice(start + 1, end).trim();
    return expr ? { value: { expr }, next: end + 1 } : null;
  }
  if (char === "[") {
    const end = closingDelimiter(input, start, "]");
    if (end < 0) return null;
    const items = parseArgs(input.slice(start + 1, end), depth + 1);
    if (!items || items.length > 64) return null;
    if (
      items.every(
        (item) => typeof item === "object" && item && "expr" in item && ident.test(item.expr),
      )
    )
      return {
        value: { refs: items.map((item) => (item as { expr: string }).expr) },
        next: end + 1,
      };
    if (items.some((item) => typeof item !== "string" && typeof item !== "number")) return null;
    return { value: { list: items as Array<string | number> }, next: end + 1 };
  }
  const token = input
    .slice(start)
    .match(/^(true|false|null|-?\d+(?:\.\d+)?|[A-Za-z][A-Za-z0-9_]{0,47})/);
  if (!token) return null;
  const raw = token[1];
  if (raw === "true") return { value: true, next: start + raw.length };
  if (raw === "false") return { value: false, next: start + raw.length };
  if (raw === "null") return { value: null, next: start + raw.length };
  if (ident.test(raw) && Number.isNaN(Number(raw)))
    return { value: { expr: raw }, next: start + raw.length };
  const number = Number(raw);
  if (!Number.isFinite(number)) return null;
  return { value: number, next: start + raw.length };
}

function closingDelimiter(input: string, start: number, close: string): number {
  let depth = 0;
  let quoted = false;
  for (let i = start; i < input.length; i++) {
    if (quoted && input[i] === "\\") {
      i += 1;
      continue;
    }
    if (input[i] === '"') {
      quoted = !quoted;
      continue;
    }
    if (quoted) continue;
    if (input[i] === input[start]) depth += 1;
    if (depth > 24) return -1;
    if (input[i] === close && --depth === 0) return i;
  }
  return -1;
}

function buildNode(
  id: string,
  nodes: Map<string, { type: string; args: Arg[] }>,
  ancestors: Set<string>,
  streaming: boolean,
  budget: { count: number },
): UiNode {
  if (++budget.count > 160) throw new Error("OpenUI node limit");
  if (ancestors.has(id) || ancestors.size > 20) throw new Error("Cyclic or deep OpenUI document");
  const node = nodes.get(id);
  if (!node) throw new Error("Missing OpenUI component");
  const entry = componentCatalog[node.type];
  if (node.args.length > entry.props.length + Number(entry.children))
    throw new Error("Unsupported OpenUI arguments");
  const props: Record<string, Expression> = Object.create(null);
  for (const [index, prop] of entry.props.entries()) {
    const value = node.args[index];
    if (value === undefined) continue;
    if (typeof value === "object" && value && "refs" in value)
      throw new Error("Component references cannot be properties");
    const source =
      typeof value === "object" && value && "expr" in value
        ? value.expr
        : JSON.stringify(
            typeof value === "object" && value && "list" in value ? value.list : value,
          );
    const expression = parseExpressionAt(source, 0, { ecmaVersion: 2022 });
    if (source.slice(expression.end).trim()) throw new Error("Trailing OpenUI expression");
    props[prop] = expression as Expression;
  }
  const result: UiNode = { type: "component", name: node.type, props, children: [] };
  const childArg = entry.children ? node.args[entry.props.length] : undefined;
  if (typeof childArg === "string") result.children = [{ type: "text", value: childArg }];
  else if (typeof childArg === "object" && childArg && "refs" in childArg) {
    const path = new Set(ancestors).add(id);
    result.children = childArg.refs
      .filter((child) => {
        if (nodes.has(child)) return true;
        if (streaming) return false;
        throw new Error("Missing OpenUI component");
      })
      .map((child) => buildNode(child, nodes, path, streaming, budget));
  } else if (childArg !== undefined) throw new Error("Unsupported OpenUI children");
  return result;
}
