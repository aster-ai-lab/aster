declare module '*.svg' {
  const src: string;
  export default src;
}

declare module '*.css';

declare module '*.module.css' {
  const classes: Record<string, string>;
  export default classes;
}
