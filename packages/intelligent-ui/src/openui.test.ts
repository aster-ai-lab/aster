import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { componentNames } from "./catalog";
import { propsOf, validAnswers } from "./document";
import { compileOpenUi } from "./openui";

const budget = `root = Card("Budget", [days, total])
days = Slider("days", "Days", 5, 1, 14)
total = Metric("Total", {days * 300}, "USD")
`;

describe("OpenUI Lang", () => {
  it("paints the closed root before the fence finishes", () => {
    const partial = `root = Card("Budget", [days])\ndays = Slider("days"`;
    const result = compileOpenUi(partial, true);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.pending).toBe(true);
    expect(propsOf(result.document.nodes[0], {}).title).toBe("Budget");
    expect(result.document.initial).toEqual({});
  });

  it("calculates model-declared budget values from live control answers", () => {
    const result = compileOpenUi(budget, false);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.document.initial).toEqual({ days: 5 });
    expect(propsOf(result.document.nodes[0].children![1], { days: 3 }).value).toBe(900);
  });

  it("rejects a component outside the shared catalog", () => {
    expect(compileOpenUi('root = Widget("x")', false).ok).toBe(false);
  });

  it("rejects a call inside an expression", () => {
    expect(compileOpenUi('root = Metric("Total", {fetch("https://example.org")})', false).ok).toBe(
      false,
    );
  });

  it("lists every catalog component in the skill", () => {
    const skill = readFileSync(
      new URL("../skills/intelligent-ui/SKILL.md", import.meta.url),
      "utf8",
    );
    for (const name of componentNames) expect(skill).toContain(name);
    expect(skill).toContain("```openui");
    expect(skill).toContain("Do not invent another interface.");
    expect(skill).not.toContain("```aix-ui");
  });
});

// These cases prevent model source from escaping the closed catalog or creating ambiguous forms.
describe("OpenUI execution boundary", () => {
  it.each([
    'import x from "remote"',
    "export const x = 1",
    "<script>alert(1)</script>",
    '<Card title="MDX" />',
    "root = Card({globalThis}, [])",
    'root = Metric("X", {globalThis.fetch("x")})',
    'root = Metric("X", {true ? 1 : fetch("x")})',
    'root = Metric("X", {budget.constructor})',
    'root = Metric("X", {budget = 1})',
    'root = Metric("X", {{__proto__: {admin: true}}})',
    'root = Metric("X", {1 / 0})',
    'root = Input("constructor", "X")',
    'root = Slider("x", "X", 2, 10, 1)',
    'root = Card("X", [missing])',
    'root = Card("X", [root])',
    'root = Text("X")\nroot = Text("Y")',
    'root = Text("X" "Y")',
    'root = Text("X",)',
    'root = Text("X")\nunused = Widget("Y")',
    'root = Card("X", [a, b])\na = Input("x", "X")\nb = Input("x", "X")',
    'root = Input("topic", "Topic", "", "text", null, null, null, "yes")',
    'root = Input("count", "Count", 3, "number", 1, 10, 0)',
    'root = Input("count", "Count", 3, "number", 10, 1)',
    'root = Textarea("notes", "Notes", false)',
    'root = Select("choice", "Choice", "", ["A", "A"])',
    'root = Select("choice", "Choice", "", [""])',
  ])("rejects unsafe or ambiguous source: %s", (source) => {
    expect(compileOpenUi(source, false).ok).toBe(false);
  });

  it("requires text and consent, and refuses empty or out-of-range numeric answers", () => {
    const result = compileOpenUi(
      `root = Card("Details", [reason, agree, count, submit])
reason = Textarea("reason", "Reason", "", true, "Explain")
agree = Checkbox("agreed", "Agree", false, true)
count = Input("count", "Count", 3, "number", 1, 10)
submit = Button("submit", "Continue")`,
      false,
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    for (const values of [
      { reason: "  ", agreed: true, count: 3 },
      { reason: "Work", agreed: false, count: 3 },
      { reason: "Work", agreed: true, count: 11 },
      { reason: "Work", agreed: true, count: "" },
    ])
      expect(validAnswers(result.document.nodes, values)).toBe(false);
    expect(validAnswers(result.document.nodes, { reason: "Work", agreed: true, count: 3 })).toBe(
      true,
    );
  });

  it.each([
    ["text", "0, 0, 0"],
    ["email", "0, 0, 0"],
    ["tel", "1, 9, 1"],
  ])("leaves positional min, max, and step unchecked on a %s input", (type, bounds) => {
    const result = compileOpenUi(
      `root = Input("field", "Field", "", "${type}", ${bounds}, false, "Hint")`,
      false,
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.document.initial).toEqual({ field: "" });
  });

  it("preserves quoted model text as data without interpreting markup", () => {
    const result = compileOpenUi('root = Text("<script>& hello, world</script>")', false);
    expect(result.ok).toBe(true);
    if (result.ok)
      expect(result.document.nodes[0].children?.[0].value).toBe("<script>& hello, world</script>");
  });

  it("preserves delimiter characters inside option labels and expression strings", () => {
    const select = compileOpenUi(
      'root = Select("choice", "Choice", "A, [draft]", ["A, [draft]", "B"])',
      false,
    );
    expect(select.ok).toBe(true);
    if (select.ok) expect(select.document.initial).toEqual({ choice: "A, [draft]" });
    const metric = compileOpenUi('root = Metric("X", {"}"})', false);
    expect(metric.ok).toBe(true);
    if (metric.ok) expect(propsOf(metric.document.nodes[0], {}).value).toBe("}");
  });

  it("bounds source and component expansion", () => {
    expect(compileOpenUi("x".repeat(65537), true)).toEqual({ ok: false, error: "limit" });
    const graph = ["root = Stack([n0])"];
    for (let i = 0; i < 21; i++) graph.push(`n${i} = Stack([n${i + 1}])`);
    graph.push('n21 = Text("X")');
    expect(compileOpenUi(graph.join("\n"), false).ok).toBe(false);
    const expanded = ["root = Stack([n0])"];
    for (let i = 0; i < 12; i++) expanded.push(`n${i} = Stack([n${i + 1}, n${i + 1}])`);
    expanded.push('n12 = Text("X")');
    expect(compileOpenUi(expanded.join("\n"), false).ok).toBe(false);
  });
});
