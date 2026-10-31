import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { act, type CSSProperties, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { OfficeSurface } from '../src/office-surface';
import { OfficeSelect } from '../src/internal/features/work/editors/office-select';
import { Dialog } from '../src/internal/design-system/primitives/dialog/dialog';
import { Button } from '../src/internal/design-system/primitives/button/button';
import { IconButton } from '../src/internal/design-system/primitives/button/icon-button';
import { Field } from '../src/internal/design-system/primitives/form/field';
import { OfficeTextField, OfficeTextArea } from '../src/internal/features/work/editors/office-text-field';
import { OfficeNumberField } from '../src/internal/features/work/editors/office-number-field';
import { Tabs } from '../src/internal/design-system/primitives/navigation/tabs';
import { WorkOfficeStatusBar, WorkOfficeZoomControls } from '../src/internal/features/work/editors/work-office-chrome';
import { PresentationSlideThumbnail } from '../src/internal/features/work/editors/presentation-slide-thumbnail';
import { createWorkArtifact } from '../src/internal/features/work/work-templates';

let root: Root | undefined;
let style: HTMLStyleElement;

beforeEach(() => {
  style = document.createElement('style');
  style.textContent = [
    readFileSync(new URL('../../ui/src/styles/tokens/spec-roles.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/tokens.css', import.meta.url), 'utf8').replace('@import "@aster-ai-lab/ui/styles/spec-roles.css";', ''),
    readFileSync(new URL('../../ui/src/styles/search-select.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/design-system.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/design-system-forms.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-office-controls.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-office-chrome.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-editor.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-spreadsheet-chrome.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-presentation.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-presentation-content.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-pdf-editor.css', import.meta.url), 'utf8'),
    readFileSync(new URL('../src/styles/work-pdf-page-organizer.css', import.meta.url), 'utf8'),
  ].join('\n');
  document.head.append(style);
});

afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  style.remove();
  document.body.replaceChildren();
});

async function mount(dialog = false) {
  const picker = <OfficeSelect ariaLabel="Size" options={[{ value: 'small', label: 'Small' }]} value="small" onValueChange={() => {}} />;
  await mountControls(dialog ? <Dialog closeLabel="关闭" title="Format" onClose={() => {}}>{picker}</Dialog> : <div className="work-office-ribbon"><div className="work-office-toolbar">{picker}</div></div>);
  return document.querySelector<HTMLButtonElement>('[data-ui="office.select"] > button')!;
}

async function mountControls(content: ReactNode) {
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(<OfficeSurface style={{ '--font-family': 'Workspace UI', '--aster-ink': '#123456' } as CSSProperties}>
    {content}
  </OfficeSurface>));
}

describe('Office shared design roles', () => {
  it('keeps picker corners when Office styles also apply outside the Office surface', async () => {
    const trigger = await mount();
    document.querySelector('.aster-office')!.classList.remove('aster-office');
    expect(getComputedStyle(trigger).borderRadius).toBe('8px');
    await act(async () => trigger.click());
    expect(getComputedStyle(document.querySelector('.ui-search-select__panel')!).borderRadius).toBe('12px');
  });

  it('uses regular 13px buttons, a 30px compact target and stable pending content', async () => {
    await mountControls(<>
      <Button tone="primary" loading>Apply</Button>
      <Button size="compact" disabled>Cancel</Button>
    </>);
    const primary = document.querySelector<HTMLButtonElement>('.ds-button.primary')!;
    const compact = document.querySelector<HTMLButtonElement>('.ds-button.compact')!;
    expect(getComputedStyle(primary).fontWeight).toBe('400');
    expect(getComputedStyle(primary).minHeight).toBe('36px');
    expect(getComputedStyle(compact).minHeight).toBe('30px');
    expect(getComputedStyle(compact).fontSize).toBe('13px');
    expect(getComputedStyle(compact).borderRadius).toBe('8px');
    expect(getComputedStyle(compact).opacity).toBe('0.4');
    expect(primary.disabled).toBe(true);
    expect(primary.getAttribute('aria-busy')).toBe('true');
    expect(primary.textContent).toBe('Apply');
    expect(getComputedStyle(primary.querySelector('.ds-button-spinner')!).position).toBe('absolute');
    expect(getComputedStyle(primary.querySelector('.ds-button-content')!).opacity).toBe('0');
  });

  it('shares the 36px field role without giving nested number inputs a second frame', async () => {
    await mountControls(<>
      <Field label="Name" description="Name help"><OfficeTextField defaultValue="Draft" /></Field>
      <Field label="Notes" error="Required"><OfficeTextArea /></Field>
      <Field label="Count"><OfficeNumberField ariaLabel="Count" value={2} onValueChange={() => {}} /></Field>
    </>);
    const input = document.querySelector<HTMLInputElement>('.work-office-text-field')!;
    const textarea = document.querySelector<HTMLTextAreaElement>('textarea')!;
    const number = document.querySelector<HTMLElement>('.work-office-number-field')!;
    expect(getComputedStyle(input).height).toBe('36px');
    expect(getComputedStyle(input).fontSize).toBe('13px');
    expect(getComputedStyle(input).backgroundColor).toBe('#f5f5f5');
    expect(getComputedStyle(textarea).backgroundColor).toBe('#f5f5f5');
    expect(getComputedStyle(number).height).toBe('36px');
    expect(getComputedStyle(number.querySelector('input')!).borderTopWidth).toBe('0px');
    expect(getComputedStyle(document.querySelector('.ds-field-label')!).fontWeight).toBe('400');
    expect(getComputedStyle(document.querySelector('.ds-field-description')!).fontSize).toBe('12px');
    expect(textarea.getAttribute('aria-invalid')).toBe('true');
    expect(document.getElementById(textarea.getAttribute('aria-describedby')!)!.textContent).toBe('Required');
  });

  it('keeps the format accent with shared sizing and neutral ribbon surfaces', async () => {
    await mountControls(<>{['document', 'spreadsheet', 'presentation'].map((kind) =>
      <div key={kind} className={`work-${kind}-editor`}><div className={`work-office-ribbon work-${kind}-ribbon`}>
        <Tabs ariaLabel={kind || 'Document'} value="home" onChange={() => {}} variant="line" size="compact" className="work-office-ribbon-tabs"
          items={[{ id: 'home', label: 'Home' }, { id: 'insert', label: 'Insert' }]} />
        <div className="work-office-toolbar"><OfficeNumberField ariaLabel="Size" value={12} onValueChange={() => {}} /></div>
      </div></div>)}</>);
    const colors = ['#185abd', '#107c41', '#c43e1c'];
    let index = 0;
    for (const tab of document.querySelectorAll<HTMLElement>('[aria-selected="true"]')) {
      expect(getComputedStyle(tab).color).toBe(colors[index++]);
      expect(getComputedStyle(tab).backgroundColor).toBe('#f2f3f5');
      expect(getComputedStyle(tab).fontSize).toBe('13px');
      expect(getComputedStyle(tab).fontWeight).toBe('500');
    }
    for (const number of document.querySelectorAll<HTMLElement>('.work-office-number-field')) {
      expect(getComputedStyle(number).height).toBe('30px');
    }
  });

  it('uses the format accent for editor zoom and keeps outer slide navigation neutral', async () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') throw new Error('Expected presentation fixture');
    const content = artifact.content;
    await mountControls(<>
      {['document', 'spreadsheet', 'presentation'].map((kind) => <div key={kind} className={`work-${kind}-editor`}>
        <WorkOfficeStatusBar controls={<WorkOfficeZoomControls zoom={90} decreaseLabel="Decrease" increaseLabel="Increase" outputLabel="Zoom" sliderLabel="Zoom level" onChange={() => {}} />}>
          Saved
        </WorkOfficeStatusBar>
      </div>)}
      <div className="work-slide-thumbnail-list">
        <PresentationSlideThumbnail content={content} slide={content.slides[0]} index={0} selected slideCount={content.slides.length}
          aspectRatio="16 / 9" variant="strip" renderPreview={false} onFocus={() => {}} onSelect={() => {}} onDelete={() => false} onNavigate={() => {}} />
      </div>
    </>);
    const colors = ['#185abd', '#107c41', '#c43e1c'];
    let index = 0;
    for (const fill of document.querySelectorAll<HTMLElement>('.work-office-slider-fill')) {
      expect(getComputedStyle(fill).backgroundColor).toBe(colors[index++]);
    }
    for (const status of document.querySelectorAll<HTMLElement>('.work-office-status')) {
      expect(getComputedStyle(status).fontSize).toBe('12px');
    }
    const thumbnail = document.querySelector<HTMLElement>('[data-slide-thumbnail]')!;
    expect(getComputedStyle(thumbnail).borderTopColor).toBe('#123456');
    expect(getComputedStyle(thumbnail).backgroundColor).toBe('#f2f3f5');
    expect(getComputedStyle(thumbnail.querySelector('span')!).fontSize).toBe('12px');
  });

  it('uses canonical toolbar sizing and preserves the host font and theme across a body portal', async () => {
    const trigger = await mount();
    const controlStyle = getComputedStyle(trigger);
    expect(controlStyle.height).toBe('30px');
    expect(controlStyle.borderRadius).toBe('8px');
    expect(controlStyle.backgroundColor).toBe('#f5f5f5');
    expect(controlStyle.fontFamily).toContain('Workspace UI');
    await act(async () => trigger.click());
    const panel = document.querySelector<HTMLElement>('.ui-search-select__panel')!;
    expect(panel.closest('.aster-office-floating-root') !== null).toBe(true);
    expect(getComputedStyle(panel).borderRadius).toBe('12px');
    expect(getComputedStyle(panel).color).toBe('#123456');
    expect(getComputedStyle(panel).fontFamily).toContain('Workspace UI');
    expect(getComputedStyle(panel.querySelector('button')!).minHeight).toBe('32px');
    expect((panel.closest('.aster-office-floating-root') as HTMLElement).style.getPropertyValue('--aster-panel')).toBe('');
  });

  it('keeps PDF controls and page selection on the shared roles', async () => {
    await mountControls(<div className="work-pdf-viewer">
      <div className="work-pdf-toolbar">
        <div className="work-pdf-save"><Button size="compact">Save</Button></div>
        <IconButton label="Undo" disabled><span /></IconButton>
        <OfficeTextField className="work-pdf-page-field" aria-label="Page number" value="1" readOnly />
      </div>
      <div className="work-pdf-thumbnail-list"><button type="button" className="active"><span className="work-pdf-thumbnail-number">1</span></button></div>
      <div className="work-pdf-page-organizer-list"><button type="button" className="selected">1</button></div>
    </div>);
    const save = getComputedStyle(document.querySelector('.work-pdf-save button')!);
    expect(save.minHeight).toBe('30px');
    expect(save.fontSize).toBe('13px');
    const undo = getComputedStyle(document.querySelector('.work-pdf-toolbar .ds-icon-button')!);
    expect(undo.width).toBe('30px');
    expect(undo.height).toBe('30px');
    expect(undo.opacity).toBe('0.4');
    expect(getComputedStyle(document.querySelector('.work-pdf-page-field')!).height).toBe('30px');
    for (const page of document.querySelectorAll('.work-pdf-thumbnail-list > button, .work-pdf-page-organizer-list > button')) {
      expect(getComputedStyle(page).borderTopColor).toBe('#123456');
      expect(getComputedStyle(page).backgroundColor).toBe('#f2f3f5');
    }
    const number = getComputedStyle(document.querySelector('.work-pdf-thumbnail-number')!);
    expect(number.fontSize).toBe('12px');
    expect(number.fontWeight).toBe('500');
  });

  it('uses 36px form controls and the canonical dialog scrim', async () => {
    const trigger = await mount(true);
    expect(getComputedStyle(trigger).height).toBe('36px');
    expect(getComputedStyle(document.querySelector('.ds-dialog')!).borderRadius).toBe('12px');
    expect(getComputedStyle(document.querySelector('.ds-dialog-backdrop')!).backgroundColor).toBe('rgba(0, 0, 0, 0.2)');
  });
});
