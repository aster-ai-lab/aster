import catalog from "./catalog.generated.json";

/** Generated from the native aster-form-core component contract. */
export type CatalogEntry = { readonly props: readonly string[]; readonly children: boolean };
export const componentCatalog: Record<string, CatalogEntry> = catalog;
export const componentNames = Object.keys(componentCatalog);
