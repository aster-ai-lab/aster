import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

describe("sandbox docs retired", () => {
  it("does not ship in-tree aster sandbox documentation pages", () => {
    assert.equal(existsSync(join(root, "content/docs/sandbox")), false);
  });
});
