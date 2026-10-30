import { describe, expect, it } from 'bun:test';
import PptxGenJS from 'pptxgenjs';
import { createWorkArtifact } from '../src/internal/features/work/work-templates';
import { createPptxBlob } from '../src/internal/features/work/work-pptx-export';
import { importPptxPresentation } from '../src/internal/features/work/work-pptx-import';
import { firstDescendant, OoxmlPackage } from '../src/internal/features/work/work-ooxml-package';

describe('presentation file round trip', () => {
  it('preserves PowerPoint point sizes through repeated import and save cycles', async () => {
    const source = new PptxGenJS();
    source.layout = 'LAYOUT_WIDE';
    source.addSlide().addText([
      { text: 'Title', options: { fontSize: 32 } },
      { text: ' body', options: { fontSize: 20 } },
    ], { x: 1, y: 1, w: 10, h: 2, fontSize: 32 });
    const original = await source.write({ outputType: 'blob' }) as Blob;
    let imported = await importPptxPresentation(new File([original], 'external.pptx'));
    expect(imported.content.slides[0].elements[0].fontSize).toBeCloseTo(32 * 96 / 72);
    for (let cycle = 0; cycle < 3; cycle += 1) {
      const artifact = { ...createWorkArtifact('strategy-deck'), content: imported.content };
      const saved = await createPptxBlob(artifact, PptxGenJS);
      const archive = await OoxmlPackage.load(await saved.arrayBuffer());
      const slide = await archive.xml('ppt/slides/slide1.xml');
      const sizes = Array.from(slide.getElementsByTagName('a:rPr')).map((run) => run.getAttribute('sz'));
      expect(sizes).toEqual(['3200', '2000']);
      imported = await importPptxPresentation(new File([saved], 'saved.pptx'));
      expect(imported.content.slides[0].elements[0].fontSize).toBeCloseTo(32 * 96 / 72);
    }
  });

  it('preserves slide names, ordering and text in the actual PPTX package and importer', async () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') throw new Error('Expected presentation fixture');
    const names = ['封面 & "初稿"', '  核心判断  ', '下一步 🌟'];
    artifact.content.slides.forEach((slide, index) => { slide.name = names[index]; });
    const blob = await createPptxBlob(artifact, PptxGenJS);
    const archive = await OoxmlPackage.load(await blob.arrayBuffer());
    const storedNames: Array<string | null | undefined> = [];
    for (let index = 0; index < names.length; index += 1) {
      const document = await archive.xml(`ppt/slides/slide${index + 1}.xml`);
      storedNames.push(firstDescendant(document, 'cSld')?.getAttribute('name'));
    }
    expect(storedNames).toEqual(names);
    const imported = await importPptxPresentation(new File([blob], 'strategy.pptx', { type: blob.type }));
    expect(imported.content.slides.map((slide) => slide.name)).toEqual(names);
    expect(imported.content.slides.map((slide) => slide.elements.map((element) => element.text)))
      .toEqual(artifact.content.slides.map((slide) => slide.elements.map((element) => element.text)));
  });
});
