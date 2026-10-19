import { Window } from 'happy-dom';

const happy = new Window({ url: 'http://localhost/' });

const assign = (key: string, value: unknown) => {
  Object.defineProperty(globalThis, key, {
    value,
    configurable: true,
    writable: true,
  });
};

assign('window', happy);
assign('document', happy.document);
assign('navigator', happy.navigator);
assign('location', happy.location);
assign('history', happy.history);
assign('HTMLElement', happy.HTMLElement);
assign('Element', happy.Element);
assign('Node', happy.Node);
assign('DocumentFragment', happy.DocumentFragment);
assign('Document', happy.Document);
assign('MutationObserver', happy.MutationObserver);
assign('getComputedStyle', happy.getComputedStyle.bind(happy));
assign('requestAnimationFrame', (cb: FrameRequestCallback) =>
  happy.setTimeout(() => cb(Date.now()), 0),
);
assign('cancelAnimationFrame', (id: number) => happy.clearTimeout(id));
assign('IS_REACT_ACT_ENVIRONMENT', true);

// Mirror common constructors testing-library expects
for (const key of [
  'Text',
  'SVGElement',
  'HTMLInputElement',
  'HTMLButtonElement',
  'HTMLDivElement',
  'Event',
  'CustomEvent',
  'MouseEvent',
  'KeyboardEvent',
  'FocusEvent',
] as const) {
  const value = (happy as unknown as Record<string, unknown>)[key];
  if (value) assign(key, value);
}
