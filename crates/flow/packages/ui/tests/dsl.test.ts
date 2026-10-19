import {
  compileA3sFlowWorkflowDag,
  compileA3sFlowWorkflowDagForPublication,
  createA3sFlowDagNodeCatalog,
  createA3sFlowDagNodeRegistry,
  defineA3sFlowCustomDagNode,
  digestA3sFlowWorkflowDag,
  digestA3sFlowWorkflowDsl,
  parseA3sFlowWorkflowDslJson,
  a3sFlowDagNodeRegistry,
  type A3sFlowWorkflowDsl,
} from "../src";
import digestVectors from "./fixtures/workflow-digest-vectors.json";

function fixture(): A3sFlowWorkflowDsl {
  return {
    version: "0.7.0",
    kind: "app",
    app: { name: "order.review", mode: "workflow" },
    dependencies: [],
    workflow: {
      graph: {
        nodes: [
          { id: "start", data: { type: "flow.start" } },
          { id: "review", data: { type: "flow.step" } },
          { id: "complete", data: { type: "flow.complete" } },
        ],
        edges: [
          { id: "start-review", source: "start", target: "review" },
          { id: "review-complete", source: "review", target: "complete" },
        ],
      },
    },
  };
}

describe("A3S Flow workflow document helpers", () => {
  it("parses, plans, and digests an executable graph deterministically", () => {
    const document = fixture();
    expect(parseA3sFlowWorkflowDslJson(JSON.stringify(document))).toMatchObject(
      {
        ok: true,
        compatibility: "compatible",
      },
    );
    expect(compileA3sFlowWorkflowDag(document.workflow.graph)).toEqual({
      ok: true,
      plan: { topLevel: ["start", "review", "complete"], scopes: {} },
    });
    expect(digestA3sFlowWorkflowDsl(document)).toMatch(/^[a-f0-9]{64}$/);
  });

  it("keeps author-facing edge labels out of the execution digest", () => {
    const document = fixture();
    const labeled = structuredClone(document);
    labeled.workflow.graph.edges[0].label = "Ready for review";

    expect(digestA3sFlowWorkflowDsl(labeled)).toBe(
      digestA3sFlowWorkflowDsl(document),
    );
  });

  it("matches the Rust engine's cross-language digest vectors", () => {
    for (const vector of digestVectors) {
      expect(
        digestA3sFlowWorkflowDag(
          vector.graph as A3sFlowWorkflowDsl["workflow"]["graph"],
        ),
      ).toBe(vector.graphDigest);

      expect(
        digestA3sFlowWorkflowDsl(vector.document as A3sFlowWorkflowDsl),
      ).toBe(vector.documentDigest);
    }
  });

  it("rejects integers that cannot be represented safely by JavaScript", () => {
    const document = fixture();
    document.workflow.graph.nodes[1].data.unsafe = Number.MAX_SAFE_INTEGER + 1;

    expect(() => digestA3sFlowWorkflowDsl(document)).toThrow(/unsafe integer/);
  });

  it("rejects cyclic values without overflowing the digest walk", () => {
    const document = fixture();
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    document.workflow.graph.nodes[1].data.cyclic = cyclic;

    expect(() => digestA3sFlowWorkflowDsl(document)).toThrow(/cyclic values/);
  });

  it("rejects application modes outside the workflow contract", () => {
    const document = fixture();
    const imported = {
      ...document,
      app: { ...document.app, mode: "advanced-chat" },
    };

    expect(parseA3sFlowWorkflowDslJson(JSON.stringify(imported))).toEqual({
      ok: false,
      issues: [
        {
          code: "flow.dsl.app_mode",
          path: "app.mode",
          message: "Workflow DSL app mode must be workflow.",
        },
      ],
    });
  });

  it("rejects duplicate JSON keys, including escaped-equivalent keys", () => {
    const duplicate = `{"version":"0.7.0","kind":"app","app":{"name":"x","mode":"workflow"},"workflow":{"graph":{"nodes":[],"edges":[]}},"workflow":{"graph":{"nodes":[],"edges":[]}}}`;
    expect(parseA3sFlowWorkflowDslJson(duplicate)).toMatchObject({
      ok: false,
      issues: [{ code: "flow.dsl.invalid_json" }],
    });

    const escaped = `{"version":"0.7.0","kind":"app","app":{"name":"x","mode":"workflow"},"workflow":{"graph":{"nodes":[],"edges":[],"n\\u006fdes":[]}},"dependencies":[]}`;
    expect(parseA3sFlowWorkflowDslJson(escaped)).toMatchObject({
      ok: false,
      issues: [{ code: "flow.dsl.invalid_json" }],
    });
  });

  it("rejects JSON nesting beyond the shared boundary", () => {
    let nested = "0";
    for (let index = 0; index <= 256; index += 1) nested = `[${nested}]`;
    expect(parseA3sFlowWorkflowDslJson(nested)).toMatchObject({
      ok: false,
      issues: [{ code: "flow.dsl.invalid_json" }],
    });
  });

  it("validates custom-node publication against an explicit capability binding", () => {
    const registration = defineA3sFlowCustomDagNode({
      manifest: {
        type: "commerce.risk.score",
        display_name: "Score order risk",
        description: "Evaluate an order with the host risk service.",
        category: "custom",
        categoryLabel: "Custom nodes",
        role: "host",
        ports: {
          inputs: [
            { id: "in", label: "In", kind: "control", types: ["FlowControl"] },
          ],
          outputs: [
            {
              id: "next",
              label: "Next",
              kind: "control",
              types: ["FlowControl"],
            },
          ],
        },
        input_types: ["Json"],
        output_types: ["Number"],
        fields: [],
        outputs: [],
      },
      capability: {
        id: "commerce/risk-score",
        version: "1.2.3",
        handler: "risk.score-order",
      },
    });
    const catalog = createA3sFlowDagNodeCatalog([registration]);
    const graph = fixture().workflow.graph;
    graph.nodes.splice(2, 0, {
      id: "risk",
      data: {
        type: registration.manifest.type,
        "x-host-policy": { queue: "risk-review" },
      },
    });
    graph.edges = [
      { id: "start-risk", source: "start", target: "risk" },
      { id: "risk-review", source: "risk", target: "review" },
      { id: "review-complete", source: "review", target: "complete" },
    ];

    expect(
      compileA3sFlowWorkflowDagForPublication(
        graph,
        catalog.registry,
        catalog.capabilities,
      ),
    ).toEqual({
      ok: true,
      plan: { topLevel: ["start", "risk", "review", "complete"], scopes: {} },
    });

    const registryWithoutBindings = createA3sFlowDagNodeRegistry([
      ...a3sFlowDagNodeRegistry.list(),
      registration.manifest,
    ]);
    expect(
      compileA3sFlowWorkflowDagForPublication(graph, registryWithoutBindings),
    ).toMatchObject({
      ok: false,
      issues: [
        {
          code: "flow.dag.capability_binding_missing",
          path: "nodes.risk.data.type",
        },
      ],
    });

    expect(
      compileA3sFlowWorkflowDagForPublication(graph, catalog.registry, {
        get: () => ({
          ...registration.capability,
          version: "^1.2.3",
        }),
        require: () => ({
          ...registration.capability,
          version: "^1.2.3",
        }),
        list: () => [],
      }),
    ).toMatchObject({
      ok: false,
      issues: [
        {
          code: "flow.dag.capability_binding_invalid",
          path: "nodes.risk.data.type",
        },
      ],
    });

    const document = fixture();
    document.workflow.graph = graph;
    const source = JSON.stringify(document);
    const parsed = parseA3sFlowWorkflowDslJson(source);
    expect(parsed).toMatchObject({ ok: true });
    if (!parsed.ok) throw new Error("Custom workflow did not parse.");
    expect(parsed.document.workflow.graph.nodes[2].data).toMatchObject({
      type: registration.manifest.type,
      "x-host-policy": { queue: "risk-review" },
    });
    expect(digestA3sFlowWorkflowDsl(parsed.document)).toBe(
      digestA3sFlowWorkflowDsl(document),
    );
  });
});
