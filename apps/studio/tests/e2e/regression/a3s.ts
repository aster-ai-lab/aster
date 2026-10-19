import type { Page } from '@playwright/test'
import { expect } from '@playwright/test'

import { completeA3sOauth } from '../../../scripts/e2e/regression/a3sOauth'
import { captureA3sAuthorizationUrl, sendA3sCallbackToOwnedApp } from '../../../scripts/e2e/regression/debugBridge'
import type { RegressionApp } from './RegressionApp'
import { openSettingsSection } from './settings'

export async function ensureA3sSignedIn(app: RegressionApp, page: Page): Promise<void> {
  await openSettingsSection(page, 'Model Provider')
  await page.getByTestId('provider-list-item-a3s').click()
  const authorize = page.getByRole('button', { name: 'Authorize with A3S', exact: true })
  const logout = page.getByRole('button', { name: 'Logout', exact: true })
  await expect(authorize.or(logout).first()).toBeVisible({ timeout: 60_000 })
  if (await authorize.isVisible().catch(() => false)) {
    const authorizationUrl = await captureA3sAuthorizationUrl(app.record, () => authorize.click())
    const callback = await completeA3sOauth(authorizationUrl, {
      account: app.config.a3s.account,
      password: app.config.a3s.password
    })
    await sendA3sCallbackToOwnedApp(app.record, callback)
    await expect
      .poll(
        () =>
          page.evaluate(async () => {
            const response = await window.api.dataApi.request({
              id: `regression-a3s-${Date.now()}`,
              method: 'GET',
              path: '/providers/a3s/api-keys'
            })
            const data = response.data as { keys?: Array<{ label?: string }> } | undefined
            return data?.keys?.some((key) => key.label === 'OAuth') ?? false
          }),
        { timeout: 60_000 }
      )
      .toBe(true)
    await openSettingsSection(page, 'Model Provider')
    const a3s = page.getByTestId('provider-list-item-a3s')
    await a3s.click()
    await expect(a3s).toHaveAttribute('data-selected', 'true')
  }
  await expect(logout).toBeVisible({ timeout: 3 * 60_000 })
}

export async function addA3sModel(page: Page, model: string, tab?: string): Promise<void> {
  if (
    await page
      .getByText(model, { exact: true })
      .isVisible()
      .catch(() => false)
  )
    return
  await page.getByRole('button', { name: 'Sync models', exact: true }).click()
  const drawer = page.locator('[data-slot="page-side-panel"][role="dialog"]:visible').first()
  await expect(drawer).toBeVisible()
  if (tab) {
    const tabLocator = drawer.getByRole('tab', { name: new RegExp(tab, 'i') })
    if (await tabLocator.isVisible().catch(() => false)) await tabLocator.click()
  }
  const search = drawer.getByPlaceholder('Search models')
  await expect(search).toBeEnabled()
  await search.fill(model)
  await expect(drawer.getByText(model, { exact: true })).toBeVisible()
  const add = drawer.getByRole('button', { name: 'Add', exact: true }).first()
  if (await add.isVisible().catch(() => false)) await add.click()
  await drawer.getByRole('button', { name: 'Close', exact: true }).click()
  await expect(drawer).toBeHidden()
}
