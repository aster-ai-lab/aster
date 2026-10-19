import { aixWorkbenchCatalogueSchema, type AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { dataWorkbenchPackage } from '../../../../../use-registry/packages/data/catalogue.mjs'

export const DATA_PACKAGE_ID = 'use/a3s/data'

/** Install slug of the first-party data package. */
export const DATA_PACKAGE_SLUG = 'a3s-data'

/** Studio plugin id. The ActivityBar lists this menu only when the member turns it on. */
export const DATA_ACTIVITY_PLUGIN_ID = 'aix.a3s-data'

const parsed = aixWorkbenchCatalogueSchema.parse({
  protocolVersion: 1,
  revision: '',
  packages: [dataWorkbenchPackage]
})

export function dataWorkbenchCatalogue(): AixWorkbenchCatalogue {
  return parsed
}
