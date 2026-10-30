/** Project an explicitly declared data form into root/elements/state JSON Render. */
import { identifier, isRecord, type Value, type Values } from "./document";

const truncate = (value: string, max: number) => [...value].slice(0, max).join("");

const protocol = new Set([
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

export function jsonRenderFromJson(source: string): string | null {
  if (new TextEncoder().encode(source).byteLength > 65536) return null;
  let raw: unknown;
  try {
    raw = JSON.parse(source);
  } catch {
    return null;
  }
  if (isRecord(raw) && isRecord(raw.elements)) return source;
  if (
    !isRecord(raw) ||
    typeof raw.root !== "string" ||
    !raw.root.trim() ||
    raw.elements !== undefined ||
    !isRecord(raw.state) ||
    !isRecord(raw.state.form) ||
    !Object.keys(raw.state.form).length
  )
    return null;
  const labels = isRecord(raw.state.labels) ? raw.state.labels : {};
  const title =
    typeof raw.title === "string" && raw.title.trim() ? raw.title.trim() : raw.root.trim();
  const elements: Record<
    string,
    { type: string; props: Record<string, Value>; children?: string[] }
  > = {};
  const form: Values = {};
  let rejected = false;
  const add = (type: string, props: Record<string, Value>, children?: string[]) => {
    const id = `n${Object.keys(elements).length}`;
    elements[id] = { type, props, ...(children ? { children } : {}) };
    return id;
  };
  const walk = (body: Record<string, unknown>, depth: number): string[] => {
    const children: string[] = [];
    for (const [key, rawValue] of Object.entries(body).slice(0, 12)) {
      if (Object.keys(elements).length >= 24) break;
      if (
        protocol.has(key) ||
        (["title", "name", "label"].includes(key) && typeof rawValue === "string")
      )
        continue;
      const labeled =
        isRecord(rawValue) &&
        typeof rawValue.label === "string" &&
        Object.hasOwn(rawValue, "value") &&
        !["options", "min", "max"].some((name) => Object.hasOwn(rawValue, name));
      const label = (
        labeled ? rawValue.label : typeof labels[key] === "string" ? labels[key] : key
      ) as string;
      const value = labeled ? rawValue.value : rawValue;
      let type: string | undefined;
      let initial = value;
      const props: Record<string, Value> = { label: truncate(label.trim(), 80) };
      if (typeof value === "string" && value.trim()) {
        type = value.includes("\n") || value.length > 80 ? "Textarea" : "Input";
        initial = truncate(value.trim(), 500);
        if (type === "Input") props.type = "text";
      } else if (typeof value === "number" && Number.isFinite(value)) {
        type = "Input";
        props.type = "number";
      } else if (typeof value === "boolean") type = "Checkbox";
      else if (
        isRecord(value) &&
        typeof (value.value ?? value.initial) === "number" &&
        typeof value.min === "number" &&
        typeof value.max === "number"
      ) {
        initial = value.value ?? value.initial;
        if (value.min < value.max && Number(initial) >= value.min && Number(initial) <= value.max) {
          type = "Slider";
          props.min = value.min;
          props.max = value.max;
          if (typeof value.step === "number") props.step = value.step;
        }
      } else if (isRecord(value) && Array.isArray(value.options)) {
        initial = value.value ?? value.initial;
        if (
          typeof initial === "string" &&
          value.options.length > 0 &&
          value.options.length <= 8 &&
          value.options.every((option) => typeof option === "string" && option.trim()) &&
          new Set(value.options).size === value.options.length &&
          value.options.includes(initial)
        ) {
          type = "Select";
          props.options = value.options as string[];
        }
      }
      if (type) {
        if (!identifier(key) || ["root", "submit"].includes(key) || Object.hasOwn(form, key)) {
          rejected = true;
          continue;
        }
        props[type === "Checkbox" ? "checked" : "value"] = { $bindState: `/form/${key}` };
        form[key] = initial as Values[string];
        children.push(add(type, props));
      } else if (isRecord(value) && depth < 2) {
        const nested = walk(value, depth + 1);
        if (nested.length) children.push(add("Card", { title: label }, nested));
      } else if (Array.isArray(value) && depth < 2 && value.length >= 2 && value.every(isRecord)) {
        for (const item of value.slice(0, 4)) {
          const nested = walk(item, depth + 1);
          if (nested.length)
            children.push(
              add(
                "Card",
                { title: String(item.title ?? item.name ?? item.label ?? children.length + 1) },
                nested,
              ),
            );
        }
      }
    }
    return children;
  };
  const children = walk(raw.state.form, 0);
  if (rejected || !Object.keys(form).length || Object.keys(elements).length > 24) return null;
  elements.submit = { type: "Button", props: { action: "submit", label: "提交" } };
  elements.root = {
    type: "Card",
    props: { title: truncate(title, 80) },
    children: [...children, "submit"],
  };
  return JSON.stringify({ root: "root", elements, state: { form } });
}
