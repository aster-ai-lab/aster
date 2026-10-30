import { Button } from "@a3s-lab/ui/components/primitives/button";
import { Checkbox } from "@a3s-lab/ui/components/primitives/checkbox";
import { Input } from "@a3s-lab/ui/components/primitives/input";
import { SearchSelect } from "@a3s-lab/ui/components/console/SearchSelect";
import { Input as Textarea } from "@a3s-lab/ui/components/primitives/textarea";
import { Slider } from "@a3s-lab/ui/components/primitives/slider";
import { useMemo, useState, type ReactNode } from "react";

import { holdsControl, validAnswers, type UiNode, type Values } from "./document";
import { compileJsonRender } from "./jsonRender";

export type SubmitOutcome = "sent" | "busy" | "failed";
export type IntelligentUIProps = {
  source: string;
  streaming: boolean;
  submit?: (values: Values) => Promise<SubmitOutcome>;
  labels: {
    invalid: string;
    pending: string;
    unavailable: string;
    sent: string;
    busy: string;
    failed: string;
    required: string;
    submitting?: string;
  };
};

const label13 = "text-[13px] leading-5 text-[#3f3f3f]";
const isAction = (node: UiNode) => node.name === "Button";

export function IntelligentUI({ source, streaming, submit, labels }: IntelligentUIProps) {
  const doc = useMemo(() => {
    const compiled = compileJsonRender(source, streaming);
    return compiled.ok
      ? { ...compiled.document, error: undefined }
      : { nodes: [], initial: {}, pending: false, error: compiled.error };
  }, [source, streaming]);
  const [edited, setEdited] = useState<Values>({});
  const [phase, setPhase] = useState<"ready" | "sending" | "sent">("ready");
  const [notice, setNotice] = useState<SubmitOutcome | "required" | null>(null);
  const values: Values = Object.fromEntries(
    Object.entries(doc.initial).map(([name, initial]) => [name, edited[name] ?? initial]),
  );
  const change = (name: string, value: string | number | boolean) => {
    setEdited((current) => ({ ...current, [name]: value }));
    setNotice(null);
  };
  const onSubmit = async () => {
    if (!submit || streaming || phase !== "ready") return;
    if (!validAnswers(doc.nodes, values)) {
      setNotice("required");
      return;
    }
    setPhase("sending");
    let result: SubmitOutcome;
    try {
      result = await submit(values);
    } catch {
      result = "failed";
    }
    setPhase(result === "sent" ? "sent" : "ready");
    setNotice(result);
  };
  const disabled = phase !== "ready";
  const noticeTone = notice === "sent" ? "text-[#8a8a8a]" : "text-[#b42318]";
  const noticeLine = notice ? (
    <p role="status" className={`text-[12px] leading-5 ${noticeTone}`}>
      {labels[notice]}
    </p>
  ) : null;
  const unavailableLine =
    !submit && doc.nodes.length ? (
      <p className="text-[12px] leading-5 text-[#8a8a8a]">{labels.unavailable}</p>
    ) : null;
  // A card that holds the submit action owns the status line beside it.
  const cardFooter = doc.nodes.some(
    (node) => node.name === "Card" && node.children?.some(isAction),
  );

  const render = (node: UiNode, key: string): ReactNode => {
    if (node.type === "text") return node.value;
    if (node.name === "Card") {
      const body = (node.children ?? []).filter((child) => !isAction(child));
      const actions = (node.children ?? []).filter(isAction);
      const title = String(node.props?.title ?? "");
      // A form reads top to bottom at a readable measure; a display card may use the full column.
      const form = holdsControl(node);
      return (
        <section
          key={key}
          className={`rounded-[14px] border border-[#ececec] bg-white px-5 pt-4 pb-5 ${form ? "w-full max-w-[520px]" : ""}`}
        >
          {title ? (
            <h3 className="mb-4 text-[14px] leading-5 font-medium text-[#1a1a1a]">{title}</h3>
          ) : null}
          <div className="flex flex-col gap-4">
            {body.map((child, i) => render(child, `${key}.b${i}`))}
          </div>
          {actions.length ? (
            <div className="mt-5 flex items-center justify-end gap-3">
              <div className="mr-auto min-w-0">{noticeLine ?? unavailableLine}</div>
              {actions.map((child, i) => render(child, `${key}.a${i}`))}
            </div>
          ) : null}
        </section>
      );
    }
    const children = node.children?.map((child, i) => render(child, `${key}.${i}`));
    const p = node.props ?? {};
    const name = String(p.name ?? "");
    const label = typeof p.label === "string" ? p.label : "";
    const mark =
      p.required === true ? (
        <span aria-hidden className="ml-0.5 text-[#b42318]">
          *
        </span>
      ) : null;
    switch (node.name) {
      case "Form":
      case "Stack":
        return (
          <div
            key={key}
            className={
              node.name === "Stack" && p.direction === "horizontal" && !holdsControl(node)
                ? "flex flex-row gap-4"
                : "flex flex-col gap-4"
            }
            style={typeof p.gap === "number" ? { gap: p.gap } : undefined}
          >
            {children}
          </div>
        );
      case "Grid":
        // Form fields take one row each; side-by-side columns are for metrics and charts.
        if (holdsControl(node))
          return (
            <div key={key} className="flex flex-col gap-4">
              {children}
            </div>
          );
        return (
          <div
            key={key}
            className="grid items-end gap-x-4 gap-y-4"
            style={{ gridTemplateColumns: `repeat(${p.columns ?? 2}, minmax(0, 1fr))` }}
          >
            {children}
          </div>
        );
      case "Text":
        return (
          <div key={key} className={label13}>
            {String(p.content ?? "")}
          </div>
        );
      case "Input":
        return (
          <label key={name} className="flex min-w-0 flex-col gap-1.5">
            <span className={label13}>
              {label}
              {mark}
            </span>
            <Input
              aria-label={label}
              disabled={disabled}
              type={typeof p.type === "string" ? p.type : "text"}
              aria-required={p.required === true}
              placeholder={p.placeholder as string | undefined}
              value={String(values[name])}
              min={p.min as number | undefined}
              max={p.max as number | undefined}
              step={p.step as number | undefined}
              onChange={(event) => {
                if (p.type !== "number") {
                  change(name, event.target.value.slice(0, 4096));
                  return;
                }
                const value = event.target.valueAsNumber;
                if (event.target.value === "") {
                  change(name, "");
                  return;
                }
                if (Number.isFinite(value))
                  change(
                    name,
                    Math.min(
                      typeof p.max === "number" ? p.max : Infinity,
                      Math.max(typeof p.min === "number" ? p.min : -Infinity, value),
                    ),
                  );
              }}
            />
          </label>
        );
      case "Textarea":
        return (
          <label key={name} className="flex min-w-0 flex-col gap-1.5">
            <span className={label13}>
              {label}
              {mark}
            </span>
            <Textarea
              aria-label={label}
              aria-required={p.required === true}
              placeholder={p.placeholder as string | undefined}
              disabled={disabled}
              value={String(values[name])}
              onChange={(event) => change(name, event.target.value.slice(0, 4096))}
            />
          </label>
        );
      case "Slider":
        return (
          <div key={name} className="flex min-w-0 flex-col gap-2.5">
            <div className={`flex justify-between ${label13}`}>
              <span>{label}</span>
              <output className="text-[#171717] tabular-nums">{String(values[name])}</output>
            </div>
            <Slider
              aria-label={label}
              disabled={disabled}
              value={[Number(values[name])]}
              min={Number(p.min)}
              max={Number(p.max)}
              step={typeof p.step === "number" ? p.step : 1}
              onValueChange={([value]) => change(name, value)}
            />
          </div>
        );
      case "Checkbox":
        return (
          <label key={name} className="flex w-fit cursor-pointer items-center gap-2.5">
            <Checkbox
              aria-label={label}
              disabled={disabled}
              checked={values[name] === true}
              onCheckedChange={(checked) => change(name, checked === true)}
            />
            <span className={`${label13} text-[#171717]`}>
              {label}
              {mark}
            </span>
          </label>
        );
      case "Select":
        return (
          <div key={name} className="flex min-w-0 flex-col gap-1.5">
            <div className={label13}>
              {label}
              {mark}
            </div>
            <SearchSelect
              disabled={disabled}
              aria-label={label}
              value={String(values[name])}
              placeholder={p.placeholder as string | undefined}
              options={(p.options as string[]).map((option) => ({ value: option, label: option }))}
              onChange={(value) => change(name, value ?? "")}
            />
          </div>
        );
      case "Metric":
        return (
          <div key={key} className="rounded-[8px] bg-[#f5f5f5] px-3 py-2.5">
            <div className="text-[12px] leading-5 text-[#8a8a8a]">{label}</div>
            <output
              aria-label={label}
              className="text-[22px] leading-7 font-semibold text-[#1a1a1a]"
            >
              {typeof p.value === "number"
                ? new Intl.NumberFormat().format(p.value)
                : String(p.value ?? "")}
              <span className="ml-1 text-sm font-normal">{String(p.unit ?? "")}</span>
            </output>
          </div>
        );
      case "BarChart": {
        const numbers = p.values as number[],
          labels = p.labels as string[];
        const max = Math.max(1, ...numbers);
        return (
          <div key={key} className="flex flex-col gap-2 text-[12px] leading-5 text-[#3f3f3f]">
            {labels.map((text, i) => (
              <div key={i} className="grid grid-cols-[80px_1fr_70px] items-center gap-2">
                <span className="truncate">{text}</span>
                <div className="h-2 rounded-full bg-[#f2f3f5]">
                  <div
                    className="h-2 rounded-full bg-[#3f3f3f]"
                    style={{ width: `${Math.max(0, Math.min(100, (numbers[i] / max) * 100))}%` }}
                  />
                </div>
                <output className="text-right tabular-nums">
                  {new Intl.NumberFormat().format(numbers[i])}
                </output>
              </div>
            ))}
          </div>
        );
      }
      case "Button": {
        const blocked = !submit || streaming || disabled;
        // A native disabled button is omitted from the webview accessibility tree.
        return (
          <Button
            key={key}
            type="button"
            size="sm"
            className={blocked ? "rounded-[8px] px-3.5 opacity-40" : "rounded-[8px] px-3.5"}
            loading={phase === "sending"}
            disabled={false}
            aria-disabled={blocked || undefined}
            tabIndex={blocked ? -1 : undefined}
            aria-label={buttonText(label)}
            onClick={() => {
              if (blocked) return;
              void onSubmit();
            }}
          >
            {buttonText(label)}
          </Button>
        );
      }
      default:
        return null;
    }
  };
  const buttonText = (label: string) =>
    phase === "sent"
      ? labels.sent
      : phase === "sending" && labels.submitting
        ? labels.submitting
        : label;

  if (doc.error)
    return (
      <p role="alert" className="text-[13px] leading-5 text-[#8a8a8a]">
        {labels.invalid}
      </p>
    );
  return (
    <div className="flex flex-col gap-3">
      {doc.nodes.map((node, i) => render(node, String(i)))}
      {doc.pending && !doc.nodes.length ? (
        <p role="status" className="text-[12px] leading-5 text-[#8a8a8a]">
          {labels.pending}
        </p>
      ) : null}
      {cardFooter ? null : (noticeLine ?? unavailableLine)}
    </div>
  );
}
