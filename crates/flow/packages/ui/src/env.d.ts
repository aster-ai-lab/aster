declare module '@aster-ai-lab/ce/basecoat';
declare module '@aster-ai-lab/ce/code-editor';
declare module '@aster-ai-lab/ce/select';

interface Window {
  basecoat?: {
    init: (name?: string) => void;
    refresh: (element: HTMLElement) => void;
    start: () => void;
  };
}
