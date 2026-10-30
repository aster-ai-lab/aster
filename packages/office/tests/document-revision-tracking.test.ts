import { afterAll, beforeAll, describe, expect, it } from 'bun:test';
import { createRequire } from 'node:module';
import JSZip from 'jszip';
import { createArtifactBlob, importOfficeFile } from '../src/core';
import { prepareDocxImport } from '../src/internal/features/work/work-docx-import';
import { attribute, descendants, firstDescendant, OoxmlPackage } from '../src/internal/features/work/work-ooxml-package';

// Happy DOM does not implement the detached-document :scope selector used by the real exporter.
const require = createRequire(new URL('../../ui/package.json', import.meta.url));
const { JSDOM } = require('jsdom');
const dom = new JSDOM('');
const globals = ['window', 'document', 'DOMParser', 'XMLSerializer', 'HTMLElement', 'Element', 'Node', 'Text', 'Document', 'DocumentFragment', 'NodeFilter'] as const;
const previous = Object.fromEntries(globals.map((key) => [key, globalThis[key]]));
beforeAll(() => {
  for (const key of globals) Object.defineProperty(globalThis, key, { configurable: true, writable: true, value: key === 'window' ? dom.window : dom.window[key] });
});
afterAll(() => {
  Object.assign(globalThis, previous);
  dom.window.close();
});

const word = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const values = [
  ['', false], ['<w:trackRevisions/>', true],
  ['<w:trackRevisions w:val="true"/>', true], ['<w:trackRevisions w:val="1"/>', true],
  ['<w:trackRevisions w:val="on"/>', true], ['<w:trackRevisions w:val="false"/>', false],
  ['<w:trackRevisions w:val="0"/>', false], ['<w:trackRevisions w:val="off"/>', false],
] as const;

async function documentFile(setting: string, revision: boolean) {
  const zip = new JSZip();
  zip.file('[Content_Types].xml', `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>`);
  zip.file('_rels/.rels', `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>`);
  zip.file('word/_rels/document.xml.rels', `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>`);
  zip.file('word/document.xml', `<w:document xmlns:w="${word}"><w:body><w:p><w:r><w:t>Original</w:t></w:r></w:p>${revision ? '<w:p><w:ins w:id="7" w:author="Reviewer" w:date="2026-10-01T12:00:00Z"><w:r><w:t>Reviewed</w:t></w:r></w:ins></w:p>' : ''}<w:sectPr/></w:body></w:document>`);
  zip.file('word/settings.xml', `<w:settings xmlns:w="${word}">${setting}</w:settings>`);
  return new File([await zip.generateAsync({ type: 'uint8array' })], 'tracking.docx', {
    type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  });
}

describe('Word revision tracking settings', () => {
  it('preserves authored leading, repeated and trailing spaces through two DOCX round trips', async () => {
    const original = await documentFile('<w:trackRevisions w:val="false"/>', false);
    const zip = await JSZip.loadAsync(await original.arrayBuffer());
    const xml = new DOMParser().parseFromString(await zip.file('word/document.xml')!.async('string'), 'application/xml');
    const text = firstDescendant(xml, 't')!;
    const expected = '  Original  中文🌟  ';
    text.textContent = expected;
    text.setAttribute('xml:space', 'preserve');
    zip.file('word/document.xml', new XMLSerializer().serializeToString(xml));
    let file = new File([await zip.generateAsync({ type: 'uint8array' })], 'spaces.docx', { type: original.type });
    for (let cycle = 0; cycle < 2; cycle++) {
      const artifact = await importOfficeFile(file);
      const blob = await createArtifactBlob(artifact);
      const archive = await OoxmlPackage.load(await blob.arrayBuffer());
      const saved = await archive.xml('word/document.xml');
      expect(descendants(saved, 't').map((node) => node.textContent).join('')).toBe(expected);
      file = new File([blob], 'spaces.docx', { type: blob.type });
    }
  });

  for (const revision of [false, true]) {
    it.each(values)(`reads %s with existing revisions=${revision}`, async (setting, expected) => {
      const prepared = await prepareDocxImport(await (await documentFile(setting, revision)).arrayBuffer());
      expect(prepared.trackChanges).toBe(expected);
      expect(prepared.changeMarkers.changes.length).toBe(revision ? 1 : 0);
    });
  }
  it.each([false, true])('preserves tracking=%s and old revisions through two real DOCX round trips', async (tracking) => {
    let artifact = await importOfficeFile(await documentFile('<w:trackRevisions w:val="false"/>', true));
    if (artifact.content.type !== 'document') throw new Error('Expected document fixture');
    artifact.content.trackChanges = tracking;
    for (let cycle = 0; cycle < 2; cycle++) {
      const blob = await createArtifactBlob(artifact);
      const archive = await OoxmlPackage.load(await blob.arrayBuffer());
      const flag = firstDescendant(await archive.xml('word/settings.xml'), 'trackRevisions');
      expect(flag !== null && !['false', '0', 'off'].includes(attribute(flag, 'val') ?? '')).toBe(tracking);
      const xml = await archive.xml('word/document.xml');
      const insertions = descendants(xml, 'ins');
      expect(insertions).toHaveLength(1);
      expect(attribute(insertions[0], 'author')).toBe('Reviewer');
      expect(attribute(insertions[0], 'date')).toBe('2026-10-01T12:00:00.000Z');
      expect(descendants(insertions[0], 't').map((node) => node.textContent).join('')).toBe('Reviewed');
      expect(descendants(xml, 't').map((node) => node.textContent).join('')).toBe('OriginalReviewed');
      artifact = await importOfficeFile(new File([blob], 'tracking.docx', { type: blob.type }));
      if (artifact.content.type !== 'document') throw new Error('Expected document fixture');
      expect(Boolean(artifact.content.trackChanges)).toBe(tracking);
      const html = new DOMParser().parseFromString(artifact.content.html, 'text/html');
      expect(html.body.querySelector('ins[data-document-change]')?.textContent).toBe('Reviewed');
      expect(html.body.textContent).toContain('Original');
    }
  });
});
