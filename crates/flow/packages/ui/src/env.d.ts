declare module '@a3s-lab/ce/basecoat';
declare module '@a3s-lab/ce/code-editor';
declare module '@a3s-lab/ce/select';

interface Window {
  basecoat?: {
    init: (name?: string) => void;
    refresh: (element: HTMLElement) => void;
    start: () => void;
  };
}
