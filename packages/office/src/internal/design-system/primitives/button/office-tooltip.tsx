import {
  type ReactElement,
  type ReactNode,
  useState,
} from 'react';
import { createPortal } from 'react-dom';

type TooltipBox = {
  left: number;
  top: number;
  below: boolean;
};

/** DESIGN.md tooltip: black pill, white 13px medium, no arrow. */
export function OfficeTooltip({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}): ReactElement {
  const [box, setBox] = useState<TooltipBox | null>(null);
  const text = label.trim();

  const show = (node: HTMLElement) => {
    const rect = node.getBoundingClientRect();
    const below = rect.top < 40;
    setBox({
      left: rect.left + rect.width / 2,
      top: below ? rect.bottom + 8 : rect.top - 8,
      below,
    });
  };

  return (
    <span
      className="work-office-tooltip"
      onMouseEnter={(event) => show(event.currentTarget)}
      onMouseLeave={() => setBox(null)}
      onFocusCapture={(event) => show(event.currentTarget)}
      onBlurCapture={() => setBox(null)}
    >
      {children}
      {text && box
        ? createPortal(
            <span
              role="tooltip"
              className="work-office-tooltip-bubble"
              style={{
                left: box.left,
                top: box.top,
                transform: box.below
                  ? 'translate(-50%, 0)'
                  : 'translate(-50%, -100%)',
              }}
            >
              {text}
            </span>,
            document.body,
          )
        : null}
    </span>
  );
}
