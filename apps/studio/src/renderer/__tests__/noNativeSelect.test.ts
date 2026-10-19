import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const studioRoot = resolve(__dirname, '../../..')
const repoRoot = resolve(studioRoot, '../..')

const SOURCE = /\.(?:tsx?|jsx?|mjs|html)$/
const NOT_PRODUCT = /(?:\/__tests__\/|\.test\.|\.spec\.|\.stories\.)/
const NATIVE_PICKER = /<(?:select|datalist)[\s>/]|createElement\(\s*['"](?:select|datalist)['"]/
const RADIX_SELECT = /\bSelectTrigger\b/

// Shrink-only: these screens still use the Radix `Select` composite. It draws its own menu, so it is
// not an OS dropdown, but new pickers use `SearchSelect` (or `Combobox`). Remove a path when its
// screen moves; never add one.
const RADIX_SELECT_DEBT = [
  'src/renderer/components/BrowserImportDialog.tsx',
  'src/renderer/components/ObsidianExportDialog.tsx',
  'src/renderer/components/PermissionModeOption.tsx',
  'src/renderer/components/RichEditor/extensions/codeBlockShiki/CodeBlockNodeView.tsx',
  'src/renderer/components/chat/settings/ChatPreferenceSections.tsx',
  'src/renderer/components/history/components/HistorySourceFilter.tsx',
  'src/renderer/components/history/components/HistoryTopBar.tsx',
  'src/renderer/components/resourceCatalog/dialogs/components/DialogFormFields.tsx',
  'src/renderer/components/resourceCatalog/dialogs/components/GroupSelector.tsx',
  'src/renderer/components/resourceCatalog/dialogs/edit/AssistantEditDialog.tsx',
  'src/renderer/components/resourceCatalog/dialogs/skill/SkillMarketplaceDialog.tsx',
  'src/renderer/pages/code/components/CurrentConfigPanel.tsx',
  'src/renderer/pages/code/components/configEditPanel/tools/ConfigFieldPrimitives.tsx',
  'src/renderer/pages/org/OrgDataScopeFields.tsx',
  'src/renderer/pages/platform/SupplierEditor.tsx',
  'src/renderer/pages/settings/AppearanceSettings/AppearanceSettings.tsx',
  'src/renderer/pages/settings/ChannelsSettings/ChannelForms.tsx',
  'src/renderer/pages/settings/DataSettings/BasicDataSettings.tsx',
  'src/renderer/pages/settings/DataSettings/ObsidianSettings.tsx',
  'src/renderer/pages/settings/DeviceConnectionsSettings/DeviceConnectionsSettings.tsx',
  'src/renderer/pages/settings/FileProcessingSettings/components/PaddleOcrModelSettings.tsx',
  'src/renderer/pages/settings/FileProcessingSettings/components/ProcessorPanel.tsx',
  'src/renderer/pages/settings/McpSettings/McpServerFields.tsx',
  'src/renderer/pages/settings/ProviderSettings/ModelList/DownloadOvmsModelPopup.tsx',
  'src/renderer/pages/settings/ProviderSettings/ModelList/ModelDrawer/ModelPricingFields.tsx',
  'src/renderer/pages/settings/ProviderSettings/ModelList/ModelDrawer/ModelPurposeFields.tsx',
  'src/renderer/pages/settings/ProviderSettings/components/ProviderApiOptionsDrawer.tsx',
  'src/renderer/pages/settings/SelectionAssistantSettings/components/SelectionActionSearchModal.tsx',
  'src/renderer/pages/settings/SelectionAssistantSettings/components/SelectionActionUserModal.tsx',
  'src/renderer/pages/settings/TasksSettings.tsx',
  'src/renderer/pages/settings/UsageSettings/UsageSettings.tsx',
  'src/renderer/pages/settings/WebSearchSettings/components/WebSearchProviderSetting.tsx',
  'src/renderer/pages/settings/aligned/ArchivedTasksPage.tsx',
  'src/renderer/windows/main/onboarding/OnboardingPage.tsx',
  'src/renderer/windows/migrationV2/MigrationApp.tsx'
]

function productFiles(dir: string, out: string[] = []): string[] {
  if (!existsSync(dir)) return out
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === 'dist') continue
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) productFiles(path, out)
    else if (SOURCE.test(path) && !NOT_PRODUCT.test(path)) out.push(path)
  }
  return out
}

function matching(root: string, dirs: string[], pattern: RegExp): string[] {
  return dirs
    .flatMap((dir) => productFiles(resolve(root, dir)))
    .filter((path) => pattern.test(readFileSync(path, 'utf8')))
    .map((path) => relative(root, path))
    .sort()
}

describe('product pickers are drawn by the design system', () => {
  it('recognises OS dropdowns in markup and in DOM construction', () => {
    for (const sample of ['<select value={x}>', '<select\n  id="a"', '<datalist id="d">', "createElement('select')"]) {
      expect(sample).toMatch(NATIVE_PICKER)
    }
    for (const sample of ['<SearchSelect', 'user-select: none', '<selection>', "createElement('section')"]) {
      expect(sample).not.toMatch(NATIVE_PICKER)
    }
  })

  it('renders no OS dropdown in Studio or the shared UI packages', () => {
    expect(matching(studioRoot, ['src/renderer'], NATIVE_PICKER)).toEqual([])
    expect(matching(repoRoot, ['packages/ui/src', 'packages/intelligent-ui/src'], NATIVE_PICKER)).toEqual([])
  })

  it('only shrinks the screens that still use the Radix Select composite', () => {
    expect(matching(studioRoot, ['src/renderer'], RADIX_SELECT)).toEqual(RADIX_SELECT_DEBT)
  })
})
