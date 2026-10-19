const COLOR_PROPERTIES = [
  'color',
  'background-color',
  'border-top-color',
  'border-right-color',
  'border-bottom-color',
  'border-left-color',
  'outline-color',
  'text-decoration-color',
  'column-rule-color',
  'fill',
  'stroke',
] as const;

/** html2canvas 1.x accepts sRGB; resolve modern CSS colors in the owned clone. */
export function normalizeWorkPdfCaptureColors(clone: Document): void {
  const canvas = clone.createElement('canvas');
  canvas.width = canvas.height = 1;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  const view = clone.defaultView;
  if (!context || !view)
    throw new Error('PDF color conversion is unavailable.');
  propagateWorkPdfTextDecorations(clone);
  for (const element of [
    clone.documentElement,
    ...clone.querySelectorAll<HTMLElement>('body, body *'),
  ]) {
    const computed = view.getComputedStyle(element);
    for (const property of COLOR_PROPERTIES) {
      const value = computed.getPropertyValue(property);
      if (!/\b(?:lab|lch|oklab|oklch|color)\(/i.test(value)) continue;
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = value;
      context.fillRect(0, 0, 1, 1);
      const [r, g, b, a] = context.getImageData(0, 0, 1, 1).data;
      element.style.setProperty(
        property,
        `rgba(${r}, ${g}, ${b}, ${a / 255})`,
        'important',
      );
    }
  }
}

/** CSS text decorations propagate through inline descendants, even explicit none. */
export function propagateWorkPdfTextDecorations(clone: Document): void {
  const view = clone.defaultView;
  if (!view) return;
  const visit = (element: HTMLElement, inherited: Map<string, string>) => {
    const decorations = new Map(inherited);
    const computed = view.getComputedStyle(element);
    for (const line of computed.textDecorationLine.split(/\s+/)) {
      if (
        line === 'underline' ||
        line === 'line-through' ||
        line === 'overline'
      ) {
        decorations.set(line, computed.textDecorationColor || computed.color);
      }
    }
    // html2canvas paints decoration only from the immediate text container.
    if (
      decorations.size &&
      [...element.childNodes].some(
        (node) => node.nodeType === 3 && node.textContent,
      )
    ) {
      element.style.textDecorationLine = [...decorations.keys()].join(' ');
      element.style.textDecorationColor =
        [...decorations.values()].at(-1) ?? computed.color;
    }
    for (const child of element.children)
      visit(child as HTMLElement, decorations);
  };
  if (clone.body) visit(clone.body, new Map());
}
