// Closed deck operations for one typed aster tool result.
// Human outline and slide writes use the same two mutations.
// The renderer and this file do not share a realm: the host calls the global below.
import { baseUi, now, slideModel, storedSlide, type Deck, type StoredSlide } from "./deck";
import { acceptHomeSeed } from "./homeSeed";
import { iconUrl } from "./icons";
import { deck, setDeck } from "./state";

const GENERATE = "mcp__presenton__generate_deck";
const READ = "mcp__presenton__read_deck";
const EDIT_OUTLINE = "mcp__presenton__edit_outline";
const EDIT_SLIDE = "mcp__presenton__edit_slide";
const TOOLS = new Set([GENERATE, READ, EDIT_OUTLINE, EDIT_SLIDE]);
const SKIP = new Set(["__speaker_note__", "image_prompt", "icon_query", "__icon_url__"]);
const DEFAULT_LANGUAGE = "Chinese (Simplified - 中文, 汉语)";
const MAX_TEXT = 4_000;

export type DeckCallResult = { status: "idle" } | { status: "applied"; text: string } | { status: "refused"; text: string };

const applied = new Set<string>();

export function resetDeckControlForTests() {
  applied.clear();
}

function record(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

/** A successful tool part. Assistant prose and a failed call are not a call. */
export function deckCallFromPart(part: unknown): { id: string; name: string; args: Record<string, unknown> } | null {
  const recordPart = record(part);
  if (!recordPart || recordPart.state !== "output-available") return null;
  if (typeof recordPart.errorText === "string" && recordPart.errorText) return null;
  const output = record(recordPart.output);
  if (output?.isError === true) return null;
  const name = typeof recordPart.toolName === "string" ? recordPart.toolName : "";
  const id = typeof recordPart.toolCallId === "string" ? recordPart.toolCallId : "";
  if (!TOOLS.has(name) || !id) return null;
  return { id, name, args: record(recordPart.input) ?? {} };
}

function codePoints(value: string): number {
  return [...value].length;
}

function bounded(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const text = value.trim();
  if (!text || codePoints(text) > MAX_TEXT) return null;
  return text;
}

function slideIndex(value: unknown, length: number): number | null {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 19 || value >= length) return null;
  return value;
}

function own(id: string): Deck | null {
  return deck && id && deck.id === id ? deck : null;
}

/** The slide write the editor's PATCH uses. */
export function commitSlide(slide: Record<string, any>): { deck: Deck; index: number; stored: StoredSlide } | null {
  const presentation = typeof slide.presentation === "string" ? slide.presentation : "";
  const found = own(presentation);
  if (!found) return null;
  const index = found.slides.findIndex((entry) => entry.id === slide.id);
  if (index < 0) return null;
  const stored = storedSlide(slide);
  const slides = found.slides.map((entry, at) => (at === index ? stored : entry));
  const next = { ...found, slides, updated_at: now() };
  setDeck(next);
  return { deck: next, index, stored };
}

/** The outline write the editor's PUT uses. */
export function writeOutlines(id: string, outlines: string[]): Deck | null {
  const found = own(id);
  if (!found) return null;
  const next = { ...found, outlines, n_slides: outlines.length, updated_at: now() };
  setDeck(next);
  return next;
}

function visible(value: string): boolean {
  return Boolean(value.trim()) && iconUrl(value) === null;
}

function collectText(value: unknown, out: string[]) {
  if (typeof value === "string") {
    if (visible(value)) out.push(value.trim());
    return;
  }
  if (Array.isArray(value)) {
    for (const child of value) collectText(child, out);
    return;
  }
  const recordValue = record(value);
  if (!recordValue) return;
  for (const [key, child] of Object.entries(recordValue)) {
    if (SKIP.has(key)) continue;
    collectText(child, out);
  }
}

export function readDeckText(): string {
  if (!deck) return "";
  const slides = deck.slides.map((slide, index) => {
    const texts: string[] = [];
    collectText(slide.content, texts);
    return { index, id: slide.id, text: texts.join("\n"), note: slide.note };
  });
  return JSON.stringify({
    id: deck.id,
    content: deck.content,
    title: deck.title,
    outlines: deck.outlines,
    slides,
  });
}

function seedBody(args: Record<string, unknown>) {
  return {
    content: args.content,
    n_slides: args.n_slides === undefined ? null : args.n_slides,
    language: args.language === undefined ? DEFAULT_LANGUAGE : args.language,
    tone: args.tone === undefined ? "default" : args.tone,
    verbosity: args.verbosity === undefined ? "standard" : args.verbosity,
    include_title_slide: args.include_title_slide === undefined ? true : args.include_title_slide,
    instructions: args.instructions === undefined ? "" : args.instructions,
  };
}

function replaceFirstVisible(value: unknown, next: string): string | null {
  let previous: string | null = null;
  const visit = (node: unknown): boolean => {
    if (previous !== null || !node || typeof node !== "object") return false;
    if (Array.isArray(node)) {
      for (let index = 0; index < node.length; index += 1) {
        const child = node[index];
        if (typeof child === "string") {
          if (!visible(child)) continue;
          previous = child;
          node[index] = next;
          return true;
        }
        if (visit(child)) return true;
      }
      return false;
    }
    for (const [key, child] of Object.entries(node as Record<string, unknown>)) {
      if (SKIP.has(key)) continue;
      if (typeof child === "string") {
        if (!visible(child)) continue;
        previous = child;
        (node as Record<string, unknown>)[key] = next;
        return true;
      }
      if (visit(child)) return true;
    }
    return false;
  };
  visit(value);
  return previous;
}

function editOutline(args: Record<string, unknown>): DeckCallResult {
  if (!deck?.outlines?.length) return { status: "refused", text: "当前任务还没有演示文稿大纲。" };
  const index = slideIndex(args.index, deck.outlines.length);
  const content = bounded(args.content);
  if (index === null || !content) return { status: "refused", text: "大纲修改需要 0 到 19 的序号，以及 1 到 4000 字的新内容。" };
  const outlines = deck.outlines.slice();
  outlines[index] = content;
  writeOutlines(deck.id, outlines);
  return { status: "applied", text: "已修改当前演示文稿的一节大纲。其他节和每一页保持原样。不要另写文件。" };
}

function editSlide(args: Record<string, unknown>): DeckCallResult {
  if (!deck?.slides.length) return { status: "refused", text: "当前任务还没有演示文稿页面。" };
  const index = slideIndex(args.index, deck.slides.length);
  const text = bounded(args.text);
  if (index === null || !text || typeof args.note !== "string" || codePoints(args.note) > MAX_TEXT) {
    return { status: "refused", text: "页面修改需要 0 到 19 的序号、1 到 4000 字的正文，以及不超过 4000 字的备注。" };
  }
  const found = deck;
  const model = structuredClone(slideModel(found, found.slides[index], index));
  const previous = replaceFirstVisible(model.content, text);
  if (!previous) return { status: "refused", text: "这一页没有可替换的可见文字。" };
  model.speaker_note = args.note;
  if (record(model.content)) model.content.__speaker_note__ = args.note;
  // Hydrated runs drop markdown markers, so a raw-string swap in `ui` leaves the old text on screen.
  const rebuilt = baseUi(model.layout_group, model.layout, model.content);
  if (rebuilt) model.ui = rebuilt;
  const saved = commitSlide(model);
  if (!saved) return { status: "refused", text: "没有找到这页幻灯片。" };
  return { status: "applied", text: "已修改当前演示文稿的一页正文和备注。其他页保持原样。不要另写文件。" };
}

export function performDeckCall(name: string, args: Record<string, unknown>): DeckCallResult {
  if (name === GENERATE) {
    const result = acceptHomeSeed(seedBody(args));
    if (!result.applied) return { status: "refused", text: result.notice ?? "主题没有写入当前演示文稿。" };
    return { status: "applied", text: "已交给当前任务的演示文稿。大纲和每一页会在右侧生成。不要另写文件。" };
  }
  if (name === READ) {
    if (!deck) return { status: "refused", text: "当前任务还没有演示文稿正文。" };
    return { status: "applied", text: readDeckText() };
  }
  if (name === EDIT_OUTLINE) return editOutline(args);
  if (name === EDIT_SLIDE) return editSlide(args);
  return { status: "idle" };
}

/** One tool result. A repeat of a successful call is idle. A refusal can be retried. */
export function applyDeckToolPart(part: unknown): DeckCallResult {
  if (typeof part === "string") return { status: "idle" };
  const call = deckCallFromPart(part);
  if (!call || applied.has(call.id)) return { status: "idle" };
  const result = performDeckCall(call.name, call.args);
  if (result.status === "applied") applied.add(call.id);
  return result;
}

const target = globalThis as typeof globalThis & {
  __asterPresentonApplyDeckToolPart?: typeof applyDeckToolPart;
  __asterPresentonReadDeck?: typeof readDeckText;
};
target.__asterPresentonApplyDeckToolPart = applyDeckToolPart;
target.__asterPresentonReadDeck = readDeckText;
