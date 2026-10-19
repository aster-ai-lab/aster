import { describe, expect, it } from "vitest";

import { compileOpenUi } from "./openui";
import { openUiFormFromJson } from "./jsonForm";

const leave = `{"root":"leave_card","state":{"form":{"cm_content":"去医院","cm_type":"病假","days":2}}}`;

describe("json display form", () => {
  it("projects a declared leave card into an OpenUI form", () => {
    const source = openUiFormFromJson(leave);
    expect(source).toBe(`n0 = Input("cm_content", "cm_content", "去医院", "text")
n1 = Input("cm_type", "cm_type", "病假", "text")
n2 = Input("days", "days", 2, "number")
submit = Button("submit", "提交")
root = Card("leave_card", [n0, n1, n2, submit])`);
    const compiled = compileOpenUi(source ?? "", false);
    expect(compiled.ok).toBe(true);
  });

  it("does not invent an answer name or search prose for a card", () => {
    expect(openUiFormFromJson('{"root":"card","state":{"form":{"开始":"明天"}}}')).toBeNull();
    expect(
      openUiFormFromJson('说明\n{"root":"card","state":{"form":{"days":2}}}\n'),
    ).toBeNull();
  });

  it("uses the title and field names supplied with the document", () => {
    const named = `{
      "root": "leave_card",
      "title": "请假",
      "state": {
        "labels": { "cm_content": "事由", "cm_type": "请假类型", "cm_start_time": "开始日期", "cm_end_time": "结束日期" },
        "form": { "cm_content": "去医院", "cm_type": "病假", "cm_start_time": "2026-10-10", "cm_end_time": "2026-10-11" }
      }
    }`;
    const source = openUiFormFromJson(named);
    expect(source).toContain('root = Card("请假"');
    expect(source).toContain('Input("cm_content", "事由", "去医院", "text")');
    expect(source).toContain('"开始日期"');
    expect(source).toContain('"结束日期"');
    expect(source).not.toContain('"n0"');
  });

  it("keeps unknown card keys and honors a label on the field", () => {
    const trip = openUiFormFromJson(
      '{"root":"trip_card","state":{"form":{"city":{"label":"城市","value":"上海"}}}}',
    );
    expect(trip).toContain('Input("city", "城市", "上海", "text")');
    expect(trip).toContain('root = Card("trip_card"');
  });

  it("leaves ordinary JSON as source", () => {
    expect(openUiFormFromJson('{"ok":true,"days":2}')).toBeNull();
    expect(openUiFormFromJson('{"root":"leave_card","state":{"form":{}}}')).toBeNull();
    expect(openUiFormFromJson("not json")).toBeNull();
  });

  it("projects a second document with another label and a Select", () => {
    const source = openUiFormFromJson(
      '{"root":"order_card","title":"订货","state":{"labels":{"pack":"包装"},"form":{"pack":{"value":"箱","options":["箱","件"]}}}}',
    );
    console.log(source);
    expect(source).toContain('Select("pack", "包装", "箱", ["箱", "件"])');
    expect(source).toContain('Button("submit", "提交")');
    expect(source?.match(/Button\("submit"/g)).toHaveLength(1);
    expect(source).toContain('root = Card("订货"');
    const compiled = compileOpenUi(source ?? "", false);
    expect(compiled.ok).toBe(true);
  });

  it("returns null for a Chinese key, a second root, and a protocol key", () => {
    const chineseKey = openUiFormFromJson('{"root":"card","state":{"form":{"备注":"稍后"}}}');
    const secondRoot = openUiFormFromJson(
      '{"root":"card","state":{"form":{"qty":1,"root":"again"}}}',
    );
    const protocolKey = openUiFormFromJson('{"root":"card","state":{"form":{"status":"done"}}}');
    console.log("chinese key", chineseKey);
    console.log("second root", secondRoot);
    console.log("protocol key", protocolKey);
    expect(chineseKey).toBeNull();
    expect(secondRoot).toBeNull();
    expect(protocolKey).toBeNull();
  });
});
