"use client";

import { forwardRef, isValidElement, useCallback, useEffect, useLayoutEffect, useRef } from "react";
import type { ButtonHTMLAttributes, ReactNode, RefObject } from "react";
import { AnimatePresence, animate, motion, useIsPresent, useMotionValue, useReducedMotion } from "motion/react";
import type { TargetAndTransition, Variants } from "motion/react";
import { motionTokens } from "../lib/motion-tokens";
import styles from "./button.module.css";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg" | "icon" | "icon-sm" | "icon-lg" | "icon-navbar";

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onDrag" | "onDragEnd" | "onDragStart" | "onAnimationStart"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
}

/** Icon buttons press a little deeper, wide buttons a little less, so every size reads as the same push. */
const pressVariants: Variants = {
  pressed: (button: RefObject<HTMLButtonElement | null>) => {
    const width = button.current?.offsetWidth ?? 0;
    return { scale: width > 220 ? .985 : width && width <= 48 ? .96 : .97, transition: { duration: motionTokens.duration.instant, ease: [...motionTokens.ease.standard] } };
  },
};

const rest: TargetAndTransition = { opacity: 1, y: 0, scale: 1 };
/** One clean crossfade: the new label rises 2px as it fades in, the outgoing one fades out faster. No blur. */
const textIn: TargetAndTransition = { opacity: 0, y: 2 };
const textOut: TargetAndTransition = { opacity: 0, transition: { duration: .1, ease: [...motionTokens.ease.standard] } };
const iconIn: TargetAndTransition = { opacity: 0, scale: .85 };
const iconOut: TargetAndTransition = { opacity: 0, transition: { duration: .1, ease: [...motionTokens.ease.standard] } };
const fadeIn: TargetAndTransition = { ...rest, opacity: 0 };
const fadeOut: TargetAndTransition = { opacity: 0, transition: { duration: 0 } };
/** Scale rides the spring; opacity tweens. */
const iconEnter = { ...motionTokens.spring.snappy, opacity: { duration: motionTokens.duration.fast, ease: [...motionTokens.ease.enter] } } as const;

/** A key for the label content: text plus element names, so a new label or icon crossfades while prop-only updates stay in place. */
function labelKey(node: ReactNode): string {
  if (node == null || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number" || typeof node === "bigint") return String(node);
  if (Array.isArray(node)) return node.map(labelKey).join("");
  if (!isValidElement(node)) return "";
  const type = node.type as string | { displayName?: string; name?: string };
  return `<${typeof type === "string" ? type : type?.displayName ?? type?.name ?? ""}>${labelKey((node.props as { children?: ReactNode }).children)}`;
}

/** Keep structured content mounted when only its nested data changes. */
function isSimpleLabel(node: ReactNode): boolean {
  if (node == null || typeof node === "boolean" || typeof node === "string" || typeof node === "number" || typeof node === "bigint") return true;
  if (Array.isArray(node)) return node.every(isSimpleLabel);
  if (!isValidElement(node)) return false;
  const children = (node.props as { children?: ReactNode }).children;
  if (typeof node.type !== "string" || node.type === "svg") return children == null || node.type === "svg";
  return node.type === "span" && (children == null || typeof children === "string" || typeof children === "number");
}

/** Springs the slot to the natural width of the incoming label when it changes, so a new label never snaps the button's size.
 *  The outgoing label is popped out of flow at once, so it never holds the old width. Other resizes (a late web font, a parent reflow) jump straight to the new width, so nothing wobbles on first paint. */
function useMorphWidth(content: RefObject<HTMLElement | null>, key: string, reduced: boolean, enabled: boolean) {
  const width = useMotionValue<number | "auto">("auto");
  const lastKey = useRef(key), armedUntil = useRef(0);
  useLayoutEffect(() => {
    if (lastKey.current === key) return;
    lastKey.current = key;
    armedUntil.current = performance.now() + 700;
  }, [key]);
  useEffect(() => {
    if (!enabled) { width.jump("auto"); return; }
    const node = content.current, slot = node?.parentElement;
    if (!node || !slot || typeof ResizeObserver === "undefined") return;
    let measured = false;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const next = entry.contentRect.width;
      if (!next || !measured || reduced || performance.now() > armedUntil.current) { measured = next > 0; width.jump(next || "auto"); delete slot.dataset.morphing; return; }
      slot.dataset.morphing = "";
      animate(width, next, { ...motionTokens.spring.morph, onComplete: () => { delete slot.dataset.morphing; } });
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [content, enabled, reduced, width]);
  return width;
}

function LabelPhase({ children, icon, reduced }: { children: ReactNode; icon: boolean; reduced: boolean }) {
  const present = useIsPresent();
  return <motion.span className={styles.labelPhase} aria-hidden={present ? undefined : true} initial={reduced ? fadeIn : icon ? iconIn : textIn} animate={rest} exit={reduced ? fadeOut : icon ? iconOut : textOut} transition={reduced ? { duration: 0 } : icon ? iconEnter : { duration: motionTokens.duration.standard, ease: [...motionTokens.ease.enter] }}>{children}</motion.span>;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant = "primary", size = "md", loading = false, disabled, children, onClick, ...props },
  ref,
) {
  const reduceMotion = useReducedMotion() ?? false;
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const contentRef = useRef<HTMLSpanElement>(null);
  const aster = typeof document !== "undefined" && document.documentElement.dataset.tenant === "aster";
  const animateLabel = !aster || isSimpleLabel(children);
  const key = animateLabel ? labelKey(children) : "";
  const width = useMorphWidth(contentRef, key, reduceMotion, animateLabel && !size.startsWith("icon"));
  const setRefs = useCallback((node: HTMLButtonElement | null) => {
    buttonRef.current = node;
    if (typeof ref === "function") ref(node); else if (ref) ref.current = node;
  }, [ref]);
  // A trigger that anchors a menu, popover, or dialog keeps its rect still while pressed, so the layer never measures a scaled anchor.
  // Radix triggers (asChild) pass aria-haspopup or data-state through; the stylesheet repeats the guard.
  const popup = props["aria-haspopup"];
  const anchorsLayer = (popup !== undefined && popup !== false && popup !== "false") || props.role === "combobox" || (props as Record<string, unknown>)["data-state"] !== undefined;
  const inert = disabled || loading || props["aria-disabled"] === true || props["aria-disabled"] === "true";
  const classes = ["arc", styles.button, styles[variant], styles[size], className].filter(Boolean).join(" ");

  return (
    <motion.button
      ref={setRefs}
      tabIndex={props.tabIndex ?? 0}
      className={classes}
      disabled={disabled || loading}
      data-busy={loading || undefined}
      aria-busy={loading || undefined}
      custom={buttonRef}
      variants={pressVariants}
      whileTap={reduceMotion || anchorsLayer || inert ? undefined : "pressed"}
      transition={motionTokens.spring.snappy}
      {...props}
      aria-disabled={props["aria-disabled"] || undefined}
      onClick={(event) => {
        if (inert) {
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        onClick?.(event);
      }}
    >
      <AnimatePresence initial={false}>
        {loading ? <motion.span key="loader" data-slot="button-pending" className={styles.loader} aria-hidden="true" initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: .6 }} animate={{ opacity: 1, scale: 1 }} exit={reduceMotion ? fadeOut : { ...iconOut, scale: .8 }} transition={reduceMotion ? { duration: 0 } : iconEnter}><span className={styles.spinner} /></motion.span> : null}
      </AnimatePresence>
      <motion.span className={[styles.labelSlot, loading ? styles.loadingLabel : ""].filter(Boolean).join(" ")} style={{ width }}>
        <span ref={contentRef} data-slot="button-label" className={styles.labelContent}>
          {animateLabel ? (
            <AnimatePresence mode="popLayout" initial={false}>
              <LabelPhase key={key} icon={!/\S/.test(key.replace(/<[^>]*>/g, ""))} reduced={reduceMotion}>{children}</LabelPhase>
            </AnimatePresence>
          ) : children}
        </span>
      </motion.span>
    </motion.button>
  );
});

Button.displayName = "Button";

export default Button;
