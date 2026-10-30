import { type AnimationEvent, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '../../dist/styles.css';
import { OfficeSurface } from '../../src/office-surface';
import { Button, Dialog, DialogPresence, Field } from '../../src/internal/design-system/primitives';
import { OfficeSelect } from '../../src/internal/features/work/editors/office-select';
import { DocumentProofingDialog } from '../../src/internal/features/work/editors/document-proofing-dialog';
import type { DocumentProofingDialogPatch } from '../../src/internal/features/work/editors/document-proofing-dialog-model';
import { OfficeTextField } from '../../src/internal/features/work/editors/office-text-field';
import { OfficeMessagesProvider } from '../../src/internal/features/work/editors/office-messages-context';
import { officeMessage, resolveOfficeMessages } from '../../src/internal/i18n/office-locale';
import { useOfficeDialog } from '../../src/internal/features/work/editors/office-dialog';

const messages = resolveOfficeMessages({ locale: new URL(location.href).searchParams.get('locale') ?? undefined });

function ControlsHarness() {
  const [font, setFont] = useState('serif');
  const [dialog, setDialog] = useState<'format' | 'proofing' | null>(null);
  const [patch, setPatch] = useState<DocumentProofingDialogPatch>({});
  const proofingInvoker = useRef<HTMLButtonElement>(null);
  const [motion, setMotion] = useState<string[]>([]);
  const officeDialog = useOfficeDialog();
  const [promptResult, setPromptResult] = useState('idle');
  const recordMotion = (phase: 'start' | 'end', event: AnimationEvent) => {
    if (!(event.target instanceof HTMLElement) || !event.target.classList.contains('ds-dialog')) return;
    setMotion((current) => [...current, `${event.animationName}:${phase}:${event.elapsedTime}`]);
  };
  const choices = [
    { value: 'serif', label: 'Serif', group: 'Fonts', previewStyle: { fontFamily: 'serif' } },
    { value: 'unavailable', label: 'Unavailable', group: 'Fonts', disabled: true },
    { value: 'sans-serif', label: 'Sans serif', group: 'Fonts', meta: 'Installed', searchText: '中文字体' },
    ...Array.from({ length: 7 }, (_, index) => ({ value: `extra-${index}`, label: `Extra ${index}` })),
  ];
  const picker = <OfficeSelect ariaLabel="Font" value={font} options={choices} onValueChange={setFont} />;
  return <OfficeSurface theme="light" style={{ height: '100vh' }}>
    <div onAnimationStart={(event) => recordMotion('start', event)} onAnimationEnd={(event) => recordMotion('end', event)} style={{ padding: 24, display: 'grid', gap: 16, alignContent: 'start', width: '100%', overflow: 'hidden' }}>
      <div className="work-office-ribbon">
        <div className="work-office-toolbar">{picker}</div>
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <Button onClick={() => setDialog('format')}>Format</Button>
        <Button ref={proofingInvoker} onClick={() => setDialog('proofing')}>Proofing</Button>
        <Button onClick={async () => {
          const value = await officeDialog.prompt({ title: 'Prompt title', fieldLabel: 'Value', initialValue: 'Draft' });
          setPromptResult(value === null ? 'cancelled' : value);
        }}>Prompt</Button>
      </div>
      <output aria-label="Selected font">{font}</output>
      <output aria-label="Applied proofing">{JSON.stringify(patch)}</output>
      <output aria-label="Dialog motion" style={{ minHeight: 20 }}>{motion.join('\n')}</output>
      <output aria-label="Motion preference">{matchMedia('(prefers-reduced-motion: reduce)').matches ? 'reduce' : 'no-preference'}</output>
      <output aria-label="Prompt result">{promptResult}</output>
      <DialogPresence>{dialog === 'format' && <Dialog closeLabel={officeMessage(messages, 'office.dialog.close')} title="Format" onClose={() => setDialog(null)}>
        <div style={{ display: 'grid', gap: 16 }}>
          {picker}
          <Field label="Name"><OfficeTextField defaultValue="Draft" /></Field>
          <div style={{ display: 'flex', gap: 8 }}>
            <Button aria-label="Idle action">Apply changes</Button>
            <Button aria-label="Pending action" loading>Apply changes</Button>
          </div>
        </div>
      </Dialog>}</DialogPresence>
      <DialogPresence>{dialog === 'proofing' && <DocumentProofingDialog
        source={{ latin: { value: 'en-US', mixed: false }, eastAsia: { value: 'zh-CN', mixed: false }, bidi: { value: null, mixed: false }, noProof: { value: false, mixed: false }, selectedCharacters: 0 }}
        restoreFocusTarget={() => proofingInvoker.current}
        onApply={(next) => { setPatch(next); return true; }}
        onClose={() => setDialog(null)}
      />}</DialogPresence>
      {officeDialog.dialog}
    </div>
  </OfficeSurface>;
}

createRoot(document.getElementById('root')!).render(<OfficeMessagesProvider catalog={messages}><ControlsHarness /></OfficeMessagesProvider>);
