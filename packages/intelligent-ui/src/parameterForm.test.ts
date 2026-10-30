import { describe, expect, it } from "vitest";

import {
  acceptedParameterPrefill,
  mergeParameterPrefill,
  projectAgentParameterSchema,
  validateParameterValues,
} from "./parameterForm";

const schema = JSON.stringify({
  type: "object",
  properties: {
    topic: { type: "string", title: "主题", maxLength: 200 },
    days: { type: "integer", title: "天数", minimum: 1, maximum: 14 },
    agreed: { type: "boolean", title: "同意" },
    kind: { type: "string", title: "类型", enum: ["a", "b"] },
  },
  required: ["days", "kind"],
});

describe("projectAgentParameterSchema", () => {
  it("projects a closed schema onto JSON Render controls", () => {
    const projected = projectAgentParameterSchema(schema);
    expect(projected.ok).toBe(true);
    if (!projected.ok) return;
    expect(projected.fields.map((field) => field.name)).toEqual([
      "topic",
      "days",
      "agreed",
      "kind",
    ]);
    expect(projected.fields.find((field) => field.name === "topic")?.longText).toBe(true);
    expect(projected.fields.find((field) => field.name === "days")?.slider).toBe(true);
    const spec = JSON.parse(projected.spec);
    expect(spec.elements.n0.type).toBe("Textarea");
    expect(spec.elements.n1).toMatchObject({ type: "Slider", props: { min: 1, max: 14, step: 1 } });
    expect(spec.elements.n3).toMatchObject({ type: "Select", props: { options: ["a", "b"] } });
    expect(spec.state.form).toMatchObject({ days: 1, kind: "a" });
    expect(spec.elements.submit.props.action).toBe("submit");
  });

  it("refuses nested objects, arrays, and unknown keywords", () => {
    for (const source of [
      '{"type":"object","properties":{"body":{"type":"object"}}}',
      '{"type":"object","properties":{"tags":{"type":"array"}}}',
      '{"type":"object","properties":{"topic":{"type":"string","pattern":"x"}}}',
      '{"type":"object","properties":{}}',
    ]) {
      expect(projectAgentParameterSchema(source).ok).toBe(false);
    }
  });
});

describe("parameter values", () => {
  const projected = projectAgentParameterSchema(schema);
  if (!projected.ok) throw new Error("schema");
  const fields = projected.fields;

  it("requires the declared fields and refuses a key outside the schema", () => {
    expect(validateParameterValues(fields, { kind: "a" }).ok).toBe(false);
    expect(validateParameterValues(fields, { days: 5, kind: "a", extra: "no" }).ok).toBe(false);
    expect(validateParameterValues(fields, { days: 5, kind: "a" }).ok).toBe(true);
  });

  it("fills only empty unedited fields and ignores a late extract after an edit", () => {
    const extracted = acceptedParameterPrefill(fields, {
      days: 3,
      kind: "b",
      topic: "合同",
      body: { nested: true },
    });
    expect(extracted).toEqual({ days: 3, kind: "b", topic: "合同" });
    const edited = mergeParameterPrefill({ days: 9 }, extracted, new Set(["days"]));
    expect(edited.days).toBe(9);
    expect(edited.kind).toBe("b");
    expect(edited.topic).toBe("合同");
    const again = mergeParameterPrefill(edited, { topic: "覆盖" }, new Set());
    expect(again.topic).toBe("合同");
  });
});
