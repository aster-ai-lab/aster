import { describe, expect, it } from "vitest";

import { compileOpenUi, describeOpenUiForm } from "./openui";

/** OpenUI the decision projector emits for a platform card and a chart. */
const leaveView = `n0 = Text("cm_content: 去医院")
n1 = Text("cm_type: 病假")
n2 = Metric("days", 2, "")
root = Card("leave_card", [n0, n1, n2])`;

const leaveForm = `n0 = Input("cm_content", "cm_content", "去医院", "text")
n1 = Input("cm_type", "cm_type", "病假", "text")
n2 = Input("days", "days", 2, "number")
submit = Button("submit", "提交")
root = Card("leave_card", [n0, n1, n2, submit])`;

const chartView = `n0 = BarChart(["一", "二"], [1, 2])
n1 = Text("note: 已汇总完成")
root = Card("本周", [n0, n1])`;

describe("decision projector OpenUI", () => {
  it("renders a lexical view as text and a metric", () => {
    const result = compileOpenUi(leaveView, false);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.pending).toBe(false);
    expect(result.document.nodes.map((node) => node.name)).toEqual(["Card"]);
    expect(result.document.nodes[0].children?.map((node) => node.name)).toEqual([
      "Text",
      "Text",
      "Metric",
    ]);
  });

  it("renders a lexical form with inputs and one submit button", () => {
    const result = compileOpenUi(leaveForm, false);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.document.initial).toEqual({
      cm_content: "去医院",
      cm_type: "病假",
      days: 2,
    });
    expect(result.document.nodes[0].children?.map((node) => node.name)).toEqual([
      "Input",
      "Input",
      "Input",
      "Button",
    ]);
  });

  it("describes the card title and control labels in source order", () => {
    expect(describeOpenUiForm(leaveForm)).toEqual({
      title: "leave_card",
      fields: [
        { name: "cm_content", label: "cm_content", kind: "Input" },
        { name: "cm_type", label: "cm_type", kind: "Input" },
        { name: "days", label: "days", kind: "Input" },
      ],
    });
    expect(describeOpenUiForm("root = Card(")).toBeNull();
  });

  it("renders a chart view", () => {
    const result = compileOpenUi(chartView, false);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.document.nodes[0].children?.map((node) => node.name)).toEqual([
      "BarChart",
      "Text",
    ]);
  });
});
