import { useId, useState, type KeyboardEvent, type ReactNode } from "react";
import type { AsterFlowDesignerContext } from "../integrations/aster-flow-designer-context";

export type AsterFlowDesignerExtensionTab = "cli" | "skill" | "copilot";

export type AsterFlowDesignerExtensionSlot =
  ReactNode | ((context: AsterFlowDesignerContext) => ReactNode);

export type AsterFlowDesignerExtensionSlots = Partial<
  Record<AsterFlowDesignerExtensionTab, AsterFlowDesignerExtensionSlot>
>;

export interface AsterFlowDesignerExtensionAreaProps {
  context: AsterFlowDesignerContext;
  extensions?: AsterFlowDesignerExtensionSlots;
  activeTab?: AsterFlowDesignerExtensionTab;
  defaultTab?: AsterFlowDesignerExtensionTab;
  onTabChange?: (tab: AsterFlowDesignerExtensionTab) => void;
  tabLabels?: Partial<Record<AsterFlowDesignerExtensionTab, string>>;
  title?: string;
  ariaLabel?: string;
  className?: string;
  /** Rendered when a host did not provide a slot for the active tab. */
  fallback?: (
    tab: AsterFlowDesignerExtensionTab,
    context: AsterFlowDesignerContext,
  ) => ReactNode;
}

const defaultTabLabels: Record<AsterFlowDesignerExtensionTab, string> = {
  cli: "CLI",
  skill: "Skill",
  copilot: "Copilot",
};

function renderSlot(
  slot: AsterFlowDesignerExtensionSlot | undefined,
  context: AsterFlowDesignerContext,
): ReactNode {
  return typeof slot === "function" ? slot(context) : slot;
}

/**
 * A small, headless-friendly tab area for host-provided Flow integrations.
 *
 * The component owns tab semantics only. It intentionally does not make a
 * network request or mutate the graph; each slot receives the same immutable
 * context and can delegate actions to its host.
 */
export function AsterFlowDesignerExtensionArea({
  context,
  extensions,
  activeTab,
  defaultTab = "copilot",
  onTabChange,
  tabLabels,
  title = "Workflow extensions",
  ariaLabel = "Workflow extensions",
  className,
  fallback,
}: AsterFlowDesignerExtensionAreaProps) {
  const [uncontrolledTab, setUncontrolledTab] =
    useState<AsterFlowDesignerExtensionTab>(defaultTab);
  const currentTab = activeTab ?? uncontrolledTab;
  const tabListId = useId();
  const panelId = `${tabListId}-panel`;
  const labels = { ...defaultTabLabels, ...tabLabels };
  const tabs: AsterFlowDesignerExtensionTab[] = ["cli", "skill", "copilot"];
  const selectTab = (tab: AsterFlowDesignerExtensionTab) => {
    if (activeTab === undefined) setUncontrolledTab(tab);
    onTabChange?.(tab);
  };
  const onTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const index = tabs.indexOf(currentTab);
    let nextIndex: number | undefined;
    if (event.key === "ArrowLeft")
      nextIndex = (index + tabs.length - 1) % tabs.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % tabs.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = tabs.length - 1;
    if (nextIndex === undefined) return;
    event.preventDefault();
    const next = tabs[nextIndex];
    selectTab(next);
    document.getElementById(`${tabListId}-${next}`)?.focus();
  };
  const supplied = extensions?.[currentTab];
  const content =
    supplied !== undefined ? (
      renderSlot(supplied, context)
    ) : (
      fallback?.(currentTab, context) ?? (
        <pre data-aster-flow-designer-context="fallback">
          {context.documentJson}
        </pre>
      )
    );

  return (
    <section
      aria-label={ariaLabel}
      className={["aster-flow-designer-extensions", className]
        .filter(Boolean)
        .join(" ")}
      data-aster-flow-designer-extensions=""
    >
      <header className="aster-flow-designer-extensions__header">
        <strong>{title}</strong>
        <div
          aria-label={ariaLabel}
          className="aster-flow-designer-extensions__tabs"
          id={tabListId}
          role="tablist"
        >
          {tabs.map((tab) => (
            <button
              aria-controls={panelId}
              aria-selected={currentTab === tab}
              className={currentTab === tab ? "is-active" : undefined}
              id={`${tabListId}-${tab}`}
              key={tab}
              onClick={() => selectTab(tab)}
              onKeyDown={onTabKeyDown}
              role="tab"
              tabIndex={currentTab === tab ? 0 : -1}
              type="button"
            >
              {labels[tab]}
            </button>
          ))}
        </div>
      </header>
      <div
        aria-labelledby={`${tabListId}-${currentTab}`}
        className="aster-flow-designer-extensions__panel"
        id={panelId}
        role="tabpanel"
        tabIndex={0}
      >
        {content}
      </div>
    </section>
  );
}

export type { AsterFlowDesignerContext } from "../integrations/aster-flow-designer-context";
