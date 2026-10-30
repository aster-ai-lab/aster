import { MarkdownEditor } from "../../dist/react.js";
import "../../dist/styles.css";
import { useMemo, useState } from "react";
import { createRoot } from "react-dom/client";

function MarkdownHarness() {
  const [markdown, setMarkdown] = useState(
    new URL(location.href).searchParams.get("markdown") ?? "Alpha",
  );
  const [saved, setSaved] = useState("");
  const content = useMemo(() => ({ type: "markdown" as const, markdown }), [markdown]);
  return (
    <main style={{ height: "95vh", display: "flex", flexDirection: "column" }}>
      <MarkdownEditor
        content={content}
        presentation="note"
        preview={false}
        locale="zh-CN"
        theme="light"
        onChange={(next) => setMarkdown(next.markdown)}
        onSave={setSaved}
      />
      <textarea aria-label="序列化 Markdown" readOnly value={markdown} />
      <textarea aria-label="保存内容" readOnly value={saved} />
      <input aria-label="编辑器外输入" defaultValue="Outside" />
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<MarkdownHarness />);
