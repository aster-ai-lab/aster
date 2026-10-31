/** The @aster-ai-lab/ce web-component runtimes (frozen bundles): they attach to
 *  the window as the basecoat custom-element framework. */
declare module '@aster-ai-lab/ce/basecoat' {
  const runtime: void;
  export default runtime;
}
declare module '@aster-ai-lab/ce/select' {
  const runtime: void;
  export default runtime;
}
declare module '@aster-ai-lab/ce/code-editor' {
  const runtime: void;
  export default runtime;
}
interface Window {
  /** The basecoat custom-element runtime once @aster-ai-lab/ce/basecoat loads. */
  basecoat?: {
    init: (componentName: string) => void;
    start: () => void;
  };
}
