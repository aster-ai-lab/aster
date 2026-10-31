// What the view keeps between openings: the deck and the page it was on,
// gzip-compressed into one view-state string of at most 64 KiB.
import { reportDeckSnapshot, reportHomeDeck, store } from "../host";
import { location, type Location } from "../router";
import type { Deck } from "./deck";

const VERSION = 2;
const MAX_STATE = 64 * 1024;

export let deck: Deck | null = null;
let onOverflow: (() => void) | null = null;

function publishHomeDeckFact() {
  const holdsWork = Boolean(deck && ((deck.outlines?.length ?? 0) > 0 || deck.slides.length > 0));
  const target = globalThis as typeof globalThis & { __asterPresentonHomeDeck?: { holdsWork: boolean } };
  target.__asterPresentonHomeDeck = { holdsWork };
  reportHomeDeck(holdsWork);
  reportDeckSnapshot();
}

export function setDeck(next: Deck | null) {
  deck = next;
  publishHomeDeckFact();
  save();
}

publishHomeDeckFact();

export function whenStateOverflows(callback: () => void) {
  onOverflow = callback;
}

async function gzip(text: string): Promise<string> {
  const stream = new Blob([text]).stream().pipeThrough(new CompressionStream("gzip"));
  const bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  return btoa(binary);
}

async function gunzip(base64: string): Promise<string> {
  const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Response(stream).text();
}

async function encode(value: unknown): Promise<string> {
  return JSON.stringify({ v: VERSION, gz: await gzip(JSON.stringify(value)) });
}

let timer: ReturnType<typeof setTimeout> | undefined;
let warned = false;

export function save() {
  clearTimeout(timer);
  timer = setTimeout(() => void flush(), 300);
}

async function flush() {
  const route = location();
  let text = await encode({ route, deck });
  if (text.length > MAX_STATE && deck) {
    // Slide edits go first; every slide still regenerates from its content.
    text = await encode({ route, deck: { ...deck, slides: deck.slides.map((slide) => ({ ...slide, patch: null })) } });
    if (!warned) onOverflow?.();
    warned = true;
  }
  if (text.length > MAX_STATE) return;
  store(text);
}

/** The saved route, after the deck is back in memory. */
export async function restore(stateJSON: string | null): Promise<Location | null> {
  if (!stateJSON) return null;
  try {
    const saved = JSON.parse(stateJSON);
    if (saved?.v !== VERSION || typeof saved.gz !== "string") return null;
    const value = JSON.parse(await gunzip(saved.gz));
    deck = value.deck && typeof value.deck === "object" ? (value.deck as Deck) : null;
    publishHomeDeckFact();
    const route = value.route;
    return route && typeof route.pathname === "string" && typeof route.search === "string" ? route : null;
  } catch {
    return null;
  }
}
