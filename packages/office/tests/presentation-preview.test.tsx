import { afterEach, describe, expect, it } from 'bun:test';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import type { WorkPresentationContent } from '../src/internal/features/work/work-types';
import { PresentationPreview } from '../src/internal/features/work/editors/presentation-player';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const roots: Root[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) {
    act(() => root.unmount());
  }
  document.body.replaceChildren();
});

function render(content: WorkPresentationContent): HTMLElement {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  roots.push(root);
  act(() => root.render(<PresentationPreview content={content} />));
  return host;
}

describe("PresentationPreview", () => {
  it("shows entrance-animated slide content without starting the animation", () => {
    const host = render({
      type: "presentation",
      slides: [
        {
          id: "slide-1",
          name: "Probe",
          background: "#ffffff",
          elements: [
            {
              id: "title-1",
              type: "text",
              x: 10,
              y: 12,
              width: 80,
              height: 18,
              text: "Probe chart slide",
              fontSize: 32,
              color: "#172033",
              fill: "transparent",
              bold: false,
              align: "left",
            },
          ],
          animations: [
            {
              id: "animation-1",
              elementId: "title-1",
              effect: "fade",
              trigger: "on-click",
              durationMs: 300,
              delayMs: 0,
            },
          ],
        },
      ],
    });

    const title = host.querySelector('[data-slide-preview-element-id="title-1"]');
    expect(title?.textContent).toBe("Probe chart slide");
    expect(title?.getAttribute("data-slide-animation-state")).toBeNull();
    expect(host.textContent).toContain("1 / 1");
  });
});
