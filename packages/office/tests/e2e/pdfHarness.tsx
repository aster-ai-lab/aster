import { PDFDict, PDFDocument, PDFHexString, PDFName, PDFString, StandardFonts, degrees } from 'pdf-lib';
import { useCallback, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { PdfViewer } from '../../dist/react.js';
import '../../dist/styles.css';

const params = new URL(location.href).searchParams;
const wasmUrl = new URL('../../dist/pdfium.wasm', import.meta.url).href;
const fixture = (async () => {
  const document = await PDFDocument.create();
  const font = await document.embedFont(StandardFonts.Helvetica);
  for (let index = 0; index < 3; index += 1) {
    const page = document.addPage([420 + index, 595]);
    page.setRotation(degrees(index * 90));
    page.drawText(`Alpha page ${index + 1}`, { x: 40, y: 520, size: 20, font });
  }
  return new Blob([new Uint8Array(await document.save()).buffer], { type: 'application/pdf' });
})();

async function inspectPdf(blob: Blob) {
  const document = await PDFDocument.load(await blob.arrayBuffer());
  return document.getPages().map((page) => {
    const annotations = page.node.Annots();
    return {
      width: page.getWidth(), height: page.getHeight(), rotation: page.getRotation().angle,
      annotations: Array.from({ length: annotations?.size() ?? 0 }, (_, index) => {
        const annotation = annotations!.lookup(index, PDFDict);
        const subtype = annotation.get(PDFName.of('Subtype'));
        const contents = annotation.get(PDFName.of('Contents'));
        return {
          subtype: subtype instanceof PDFName ? subtype.decodeText() : null,
          contents: contents instanceof PDFString || contents instanceof PDFHexString ? contents.decodeText() : null,
        };
      }),
    };
  });
}

function PdfHarness() {
  const [revision, setRevision] = useState(0);
  const [page, setPage] = useState(0);
  const [loads, setLoads] = useState(0);
  const [saves, setSaves] = useState(0);
  const [acceptedSaves, setAcceptedSaves] = useState(0);
  const [saveOutcome, setSaveOutcome] = useState('idle');
  const [savedInfo, setSavedInfo] = useState<Awaited<ReturnType<typeof inspectPdf>>>();
  const [exportedInfo, setExportedInfo] = useState('none');
  const [error, setError] = useState('none');
  const [annotationInput, setAnnotationInput] = useState('none');
  const source = useRef<Blob | undefined>(undefined);
  const saved = useRef<Blob | undefined>(undefined);
  const loadCount = useRef(0);
  const saveCount = useRef(0);
  const savePolicy = useRef(params.get('save') ?? 'accept');
  useEffect(() => {
    const observeInput = (event: Event) => {
      const target = event.composedPath().find((node): node is HTMLElement => node instanceof HTMLElement && node.isContentEditable);
      if (target) setAnnotationInput(target.innerText);
    };
    document.addEventListener('input', observeInput);
    return () => document.removeEventListener('input', observeInput);
  }, []);
  const loadSource = useCallback(async () => {
    const attempt = ++loadCount.current;
    setLoads(attempt);
    if (params.get('load') === 'once' && attempt === 1) throw new Error('Synthetic source failure');
    return source.current ?? await fixture;
  }, []);
  const onSave = useCallback(async (blob: Blob) => {
    setSaves(++saveCount.current);
    if (savePolicy.current === 'reject') {
      setSaveOutcome('rejected');
      throw new Error('Synthetic save rejection');
    }
    if (savePolicy.current === 'false') {
      setSaveOutcome('refused');
      return false;
    }
    setSavedInfo(await inspectPdf(blob));
    saved.current = blob;
    setAcceptedSaves((value) => value + 1);
    setSaveOutcome('accepted');
    return true;
  }, []);
  return <main style={{ height: '98vh', display: 'flex', flexDirection: 'column' }}>
    <div style={{ flex: 1, minHeight: 0 }}>
      <PdfViewer fileName="quality.pdf" sourceKey={String(revision)} loadSource={loadSource}
        wasmUrl={wasmUrl} locale={params.get('locale') ?? 'en-US'} theme="light" themePreference="light"
        onPageChange={setPage} onError={(failure) => setError(String(failure.error))}
        onSave={params.get('readonly') === '1' ? undefined : onSave}
        onPageExport={async (files) => {
          setExportedInfo(JSON.stringify(await Promise.all(files.map(async (file) => ({ name: file.fileName, pages: await inspectPdf(file.pdf) })))));
          return true;
        }} />
    </div>
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
      <button type="button" onClick={() => { savePolicy.current = 'accept'; }}>Accept saves</button>
      <button type="button" disabled={!saved.current} onClick={() => { source.current = saved.current; setRevision((value) => value + 1); }}>Reopen saved PDF</button>
      <output aria-label="Current page">{page}</output>
      <output aria-label="Source loads">{loads}</output>
      <output aria-label="Save calls">{saves}</output>
      <output aria-label="Accepted saves">{acceptedSaves}</output>
      <output aria-label="Save outcome">{saveOutcome}</output>
      <output aria-label="Host error">{error}</output>
      <output aria-label="Saved rotations">{savedInfo?.map((page) => page.rotation).join(',') ?? 'none'}</output>
      <output aria-label="Saved widths">{savedInfo?.map((page) => page.width).join(',') ?? 'none'}</output>
    </div>
    <textarea aria-label="Saved PDF structure" readOnly value={savedInfo ? JSON.stringify(savedInfo) : 'none'} />
    <textarea aria-label="Exported PDF structure" readOnly value={exportedInfo} />
    <textarea aria-label="Annotation input" readOnly value={annotationInput} />
  </main>;
}

createRoot(document.getElementById('root')!).render(<PdfHarness />);
