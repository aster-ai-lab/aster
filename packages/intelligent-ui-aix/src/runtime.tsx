import { createRoot } from "react-dom/client";

import {
  IntelligentUI,
  type IntelligentUIProps,
  type SubmitOutcome,
} from "@a3s-lab/intelligent-ui";
type Values = Parameters<NonNullable<IntelligentUIProps["submit"]>>[0];

let port: MessagePort | null = null;
let request = 0;
let pending: { id: number; resolve: (outcome: SubmitOutcome) => void } | null = null;
const root = createRoot(document.getElementById("root") as HTMLElement);
const submit = (values: Values) =>
  new Promise<SubmitOutcome>((resolve) => {
    if (!port || pending) {
      resolve("busy");
      return;
    }
    pending = { id: ++request, resolve };
    port.postMessage({ type: "submit", id: request, values });
  });

// An opaque sandbox receives no authority token and only accepts its parent once.
window.addEventListener("message", (event) => {
  if (
    port ||
    event.source !== window.parent ||
    event.data?.type !== "a3s-intelligent-ui-connect" ||
    event.ports.length !== 1
  )
    return;
  port = event.ports[0];
  port.onmessage = ({ data }) => {
    if (data?.type === "render" && typeof data.source === "string" && data.source.length <= 65536) {
      const props = data as IntelligentUIProps & { canSubmit: boolean };
      root.render(
        <IntelligentUI
          source={props.source}
          streaming={props.streaming === true}
          labels={props.labels}
          submit={props.canSubmit ? submit : undefined}
        />,
      );
    } else if (
      data?.type === "submitted" &&
      pending &&
      pending.id === data.id &&
      ["sent", "busy", "failed"].includes(data.outcome)
    ) {
      pending.resolve(data.outcome);
      pending = null;
    }
  };
  port.start();
  port.postMessage({ type: "ready" });
  const element = document.getElementById("root") as HTMLElement;
  const observer = new ResizeObserver(() =>
    port?.postMessage({
      type: "resize",
      height: Math.ceil(element.getBoundingClientRect().height),
    }),
  );
  observer.observe(element);
});
