import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import {
  artifactExtension, createArtifact, createArtifactBlob, importOfficeFile,
  type OfficeArtifactContent,
} from '../../dist/core.js';
import {
  DocumentEditor, PresentationEditor, SpreadsheetEditor,
  defaultDocumentLayoutFonts, defaultDocumentLayoutFontUrl,
  defaultDocumentLatinLayoutFontUrl, defaultDocumentArabicLayoutFontUrl,
  defaultDocumentHebrewLayoutFontUrl,
} from '../../dist/react.js';
import '../../dist/styles.css';

const templates = new Map([
  ['document', 'project-brief'],
  ['spreadsheet', 'quarterly-plan'],
  ['presentation', 'strategy-deck'],
]);
const params = new URL(location.href).searchParams;
const template = templates.get(params.get('editor') ?? 'document');
if (!template) throw new Error('Unknown editor fixture');
const artifact = createArtifact(template);
const kernelWasmUrl = new URL('../../dist/office-kernel.wasm', import.meta.url).href;
const fontAssets = new Map([
  [defaultDocumentLayoutFontUrl, new URL('../../dist/noto-sans-hans-regular.otf', import.meta.url).href],
  [defaultDocumentLatinLayoutFontUrl, new URL('../../dist/noto-sans-regular.ttf', import.meta.url).href],
  [defaultDocumentArabicLayoutFontUrl, new URL('../../dist/noto-naskh-arabic-regular.ttf', import.meta.url).href],
  [defaultDocumentHebrewLayoutFontUrl, new URL('../../dist/noto-sans-hebrew-regular.ttf', import.meta.url).href],
]);
const layoutFonts = defaultDocumentLayoutFonts.map((font) => {
  const url = fontAssets.get(font.url);
  if (!url) throw new Error(`Missing fixture font asset: ${font.id}`);
  return { ...font, url };
});

function EditorsHarness() {
  const [content, setContent] = useState<OfficeArtifactContent>(artifact.content);
  const [reopened, setReopened] = useState<OfficeArtifactContent>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const common = {
    preview: params.get('preview') === '1',
    locale: params.get('locale') ?? 'zh-CN',
    theme: 'light' as const,
    kernelWasmUrl,
    onChange: setContent,
    ...(params.has('derived') ? { onDerivedChange: (next: OfficeArtifactContent) => setContent(next) } : {}),
    onError: (failure: { error: unknown }) => setError(String(failure.error)),
  };
  const roundTrip = async () => {
    setBusy(true);
    setError('');
    try {
      const blob = await createArtifactBlob({ ...artifact, content });
      const file = new File([blob], `fixture.${artifactExtension(artifact.kind)}`, { type: blob.type });
      const imported = await importOfficeFile(file);
      setReopened(imported.content);
      setContent(imported.content);
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };

  return <main style={{ display: 'flex', flexDirection: 'column', height: '98vh' }}>
    <div style={{ flex: 1, minHeight: 0 }}>
      {content.type === 'document' && <DocumentEditor {...common} content={content} layoutFonts={layoutFonts} />}
      {content.type === 'spreadsheet' && <SpreadsheetEditor {...common} content={content} />}
      {content.type === 'presentation' && <PresentationEditor {...common} content={content} />}
    </div>
    <div style={{ display: 'flex', gap: 8 }}>
      <button type="button" onClick={roundTrip} disabled={busy}>Export and reopen</button>
      <output aria-label="Round trip state">{busy ? 'pending' : reopened ? 'reopened' : 'idle'}</output>
      <output role="alert">{error}</output>
    </div>
    <textarea aria-label="Editor content" readOnly value={JSON.stringify(content)} />
    <textarea aria-label="Initial content" readOnly value={JSON.stringify(artifact.content)} />
    <output aria-label="Formula cache">{content.type === 'spreadsheet' ? content.sheets[0]?.data?.[3]?.[5]?.v ?? 'empty' : ''}</output>
    <output aria-label="Formula source">{content.type === 'spreadsheet' ? content.sheets[0]?.data?.[3]?.[5]?.f : ''}</output>
    <textarea aria-label="Reopened content" readOnly value={reopened ? JSON.stringify(reopened) : ''} />
  </main>;
}

async function start() {
  if (params.has('import')) {
    const blob = await createArtifactBlob(artifact);
    const file = new File([blob], `fixture.${artifactExtension(artifact.kind)}`, { type: blob.type });
    Object.assign(artifact, await importOfficeFile(file));
  }
  createRoot(document.getElementById('root')!).render(<EditorsHarness />);
}
void start();
