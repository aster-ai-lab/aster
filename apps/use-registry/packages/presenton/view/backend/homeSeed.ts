// Homepage outline seed. This accepts the fields the home composer collected.
// It does not call the model. A deck that already has an outline or slides stays.
import { LANGUAGES, MAX_DECK_SLIDES, TONES, VERBOSITIES } from "../../agent/deck.mjs";
import { now, uuid, type Deck } from "./deck";
import { navigate } from "../router";
import { deck, setDeck } from "./state";

export const HOME_SEED_KEPT_NOTICE = "当前演示文稿还在，首页草稿未写入";

export type HomeSeed = {
  content: string;
  n_slides: number | null;
  language: string;
  tone: string;
  verbosity: string;
  include_title_slide: boolean;
  instructions: string;
};

export let acceptedHomeSeed: HomeSeed | null = null;

export function resetAcceptedHomeSeedForTests() {
  acceptedHomeSeed = null;
}

const closed = (value: unknown, options: readonly string[]) =>
  typeof value === "string" && options.includes(value);

function parse(body: unknown): HomeSeed | null {
  if (!body || typeof body !== "object") return null;
  const record = body as Record<string, unknown>;
  const content = typeof record.content === "string" ? record.content.trim() : "";
  if (!content || content.length > 12_000) return null;
  const n = record.n_slides;
  if (n !== null && (typeof n !== "number" || !Number.isInteger(n) || n < 1 || n > MAX_DECK_SLIDES)) return null;
  if (!closed(record.language, LANGUAGES) || !closed(record.tone, TONES) || !closed(record.verbosity, VERBOSITIES)) return null;
  if (typeof record.include_title_slide !== "boolean") return null;
  if (typeof record.instructions !== "string" || record.instructions.length > 4000) return null;
  return {
    content,
    n_slides: n as number | null,
    language: record.language as string,
    tone: record.tone as string,
    verbosity: record.verbosity as string,
    include_title_slide: record.include_title_slide,
    instructions: record.instructions,
  };
}

export function holdsOutlineWork(current: Deck | null = deck): boolean {
  return Boolean(current && ((current.outlines?.length ?? 0) > 0 || current.slides.length > 0));
}

/** Apply one homepage seed. Returns the seed the view kept, or a notice when the deck already has work. */
export function acceptHomeSeed(body: unknown): { applied: boolean; notice?: string; seed?: HomeSeed } {
  const seed = parse(body);
  if (!seed) return { applied: false };
  if (holdsOutlineWork()) return { applied: false, notice: HOME_SEED_KEPT_NOTICE };
  const created = now();
  const next: Deck = {
    id: uuid(),
    created_at: created,
    updated_at: created,
    content: seed.content,
    n_slides: seed.n_slides ?? 0,
    language: seed.language,
    tone: seed.tone,
    verbosity: seed.verbosity,
    instructions: seed.instructions || null,
    include_title_slide: seed.include_title_slide,
    title: null,
    outlines: null,
    template: null,
    structure: null,
    slides: [],
  };
  acceptedHomeSeed = seed;
  setDeck(next);
  // The outline page opens the outline stream. An empty deck on the presentation
  // page is treated as a finished load and gets a blank slide instead.
  navigate(`/outline?id=${encodeURIComponent(next.id)}`, "replace");
  return { applied: true, seed };
}

const target = globalThis as typeof globalThis & {
  __asterPresentonAcceptHomeSeed?: typeof acceptHomeSeed;
};
target.__asterPresentonAcceptHomeSeed = acceptHomeSeed;
