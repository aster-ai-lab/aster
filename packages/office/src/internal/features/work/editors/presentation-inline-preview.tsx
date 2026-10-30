import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { officeMessage } from '../../../i18n/office-locale';
import type { WorkPresentationContent } from '../work-types';
import { useOfficeMessages } from './office-messages-context';
import { SlideCanvas } from './presentation-slide-canvas';

export function PresentationInlinePreview({ content }: { content: WorkPresentationContent }) {
  const messages = useOfficeMessages();
  const [index, setIndex] = useState(0);
  const selectedIndex = Math.min(index, content.slides.length - 1);
  const slide = content.slides[selectedIndex];
  if (!slide) return null;
  return (
    <section
      className="work-presentation-inline-preview"
      aria-label={officeMessage(messages, 'presentation.editor.previewLabel')}
    >
      <SlideCanvas
        content={content}
        slide={slide}
        interactive={false}
        aspectRatio={`${content.width ?? 13.333} / ${content.height ?? 7.5}`}
      />
      <footer>
        <button
          type="button"
          aria-label={officeMessage(messages, 'presentation.player.prevAria')}
          disabled={selectedIndex === 0}
          onClick={() => setIndex(selectedIndex - 1)}
        >
          <ChevronLeft size={14} strokeWidth={1.7} />
        </button>
        <span aria-live="polite">{selectedIndex + 1} / {content.slides.length}</span>
        <button
          type="button"
          aria-label={officeMessage(messages, 'presentation.player.nextAria')}
          disabled={selectedIndex === content.slides.length - 1}
          onClick={() => setIndex(selectedIndex + 1)}
        >
          <ChevronRight size={14} strokeWidth={1.7} />
        </button>
      </footer>
    </section>
  );
}
