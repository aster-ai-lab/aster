import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { compileJsonRender, describeJsonRenderForm, jsonRenderFromJson } from "./jsonRender";
import { validAnswers } from "./document";
import { componentNames } from "./catalog";

const form = JSON.parse(readFileSync(new URL("../fixtures/form.json", import.meta.url), "utf8"));
const compile = (value: unknown) => compileJsonRender(JSON.stringify(value), false);

describe("closed JSON Render", () => {
  it("renders the shared native fixture and preserves typed defaults and labels", () => {
    const result = compile(form);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.document.initial).toEqual({
      name: "",
      amount: 3,
      ready: false,
      choice: "beta",
      notes: "",
    });
    expect(describeJsonRenderForm(JSON.stringify(form))?.fields.map((field) => field.name)).toEqual(
      ["name", "amount", "ready", "choice", "notes"],
    );
    expect(
      validAnswers(result.document.nodes, {
        name: "Ada",
        amount: 4,
        ready: true,
        choice: "beta",
        notes: "",
      }),
    ).toBe(true);
    for (const change of [
      { name: " " },
      { ready: false },
      { amount: 6 },
      { choice: "other" },
      { amount: "" },
    ])
      expect(
        validAnswers(result.document.nodes, {
          name: "Ada",
          amount: 4,
          ready: true,
          choice: "beta",
          notes: "",
          ...change,
        }),
      ).toBe(false);
  });
  it("renders the shared layout, slider, metric and chart fixture", () => {
    const source = readFileSync(new URL("../fixtures/display.json", import.meta.url), "utf8");
    const result = compileJsonRender(source, false);
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.document.initial).toEqual({ days: 5 });
    expect(describeJsonRenderForm(source)?.title).toBe("Summary");
  });
  it("waits for complete JSON while streaming", () => {
    const source = JSON.stringify(form);
    expect(compileJsonRender(source.slice(0, -1), true)).toMatchObject({
      ok: true,
      pending: true,
      document: { nodes: [] },
    });
    expect(compileJsonRender(source, false).ok).toBe(true);
    expect(compileJsonRender(source.slice(0, -1), false).ok).toBe(false);
  });
  it.each([
    (v: typeof form) => {
      v.elements.name.props.onChange = "fetch()";
    },
    (v: typeof form) => {
      v.elements.name.type = "script";
    },
    (v: typeof form) => {
      v.elements.name.props.value.$bindState = "/secrets/token";
    },
    (v: typeof form) => {
      v.elements.name.props.value.$bindState = "/form/constructor";
    },
    (v: typeof form) => {
      v.elements.name.props.value.$bindState = "/form/amount";
    },
    (v: typeof form) => {
      v.elements.root.children.push("root");
    },
    (v: typeof form) => {
      v.elements.root.children.push("missing");
    },
    (v: typeof form) => {
      v.elements.extra = { type: "Text", props: { content: "hidden" } };
    },
    (v: typeof form) => {
      v.state.form.secret = "hidden";
    },
    (v: typeof form) => {
      v.state.form.amount = "wrong";
    },
    (v: typeof form) => {
      v.state.form.name = null;
    },
    (v: typeof form) => {
      v.elements.name.props.min = "wrong";
    },
    (v: typeof form) => {
      v.elements.name.children = null;
    },
    (v: typeof form) => {
      v.state = null;
    },
    (v: typeof form) => {
      v.elements.choice.props.options = ["beta", "beta"];
    },
    (v: typeof form) => {
      v.elements.submit.props.action = "shell";
    },
    (v: typeof form) => {
      v.elements.amount.props.min = 9;
    },
    (v: typeof form) => {
      v.elements.root.props.title = { $eval: "fetch()" };
    },
    (v: typeof form) => {
      v.elements.name.props.required = "yes";
    },
    (v: typeof form) => {
      v.elements.root = {
        type: "Stack",
        props: { direction: ["horizontal"] },
        children: v.elements.root.children,
      };
    },
  ])("refuses unsafe or ambiguous documents", (mutate) => {
    const value = structuredClone(form);
    mutate(value);
    expect(compile(value).ok).toBe(false);
  });
  it("keeps markup literal and rejects retired source languages", () => {
    expect(
      compile({
        root: "root",
        elements: { root: { type: "Text", props: { content: "<script>literal</script>" } } },
      }).ok,
    ).toBe(true);
    for (const source of ['root = Input("name", "Name")', "<Card />", 'import x from "remote"'])
      expect(compileJsonRender(source, false).ok).toBe(false);
    expect(compileJsonRender("x".repeat(65537), true)).toEqual({ ok: false, error: "limit" });
  });
  it("bounds graph depth and rejects duplicate references", () => {
    const elements: Record<string, unknown> = {};
    for (let i = 0; i < 22; i++)
      elements[`n${i}`] = { type: "Stack", props: {}, children: [`n${i + 1}`] };
    elements.n22 = { type: "Text", props: { content: "deep" } };
    expect(compile({ root: "n0", elements }).ok).toBe(false);
    const value = structuredClone(form);
    value.elements.root.children.push("name");
    expect(compile(value).ok).toBe(false);
  });
  it("documents only the JSON contract and every component", () => {
    const skill = readFileSync(
      new URL("../skills/intelligent-ui/SKILL.md", import.meta.url),
      "utf8",
    );
    for (const name of componentNames) expect(skill).toContain(name);
    expect(skill).toContain("```json-render");
    expect(skill.toLowerCase()).not.toContain("openui");
  });
});

describe("declared data forms", () => {
  it("projects labels and values directly into JSON Render", () => {
    const source = jsonRenderFromJson(
      JSON.stringify({
        root: "leave_card",
        title: "请假",
        state: {
          form: {
            reason: { label: "事由", value: "去医院" },
            days: 2,
            kind: { value: "病假", options: ["病假", "事假"] },
          },
        },
      }),
    );
    expect(source).not.toBeNull();
    const result = compileJsonRender(source!, false);
    expect(result.ok).toBe(true);
    if (result.ok)
      expect(result.document.initial).toEqual({ reason: "去医院", days: 2, kind: "病假" });
    expect(describeJsonRenderForm(source!)?.title).toBe("请假");
  });
  it("does not invent names or extract cards from prose", () => {
    for (const source of [
      '{"root":"card","state":{"form":{"备注":"稍后"}}}',
      'text {"root":"card","state":{"form":{"days":2}}}',
      '{"ok":true}',
      '{"root":"card","state":{"form":{"status":"done"}}}',
    ])
      expect(jsonRenderFromJson(source)).toBeNull();
  });
});
