import { SearchSelect } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

/** One available gateway model. */
export type CloudModelOption = {
  id: string
  label: string
}

/** Fetch available models from the gateway's /v1/models endpoint. */
async function fetchGatewayModels(gatewayOrigin: string, bearer?: string): Promise<CloudModelOption[]> {
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  if (bearer) headers.authorization = `Bearer ${bearer}`
  const response = await fetch(`${gatewayOrigin.replace(/\/$/, '')}/v1/models`, { headers })
  if (!response.ok) return []
  const body = (await response.json()) as { data?: Array<{ id: string }> }
  return (body.data ?? []).map((model) => ({
    id: model.id,
    label: model.id
  }))
}

/** Model picker for cloud agent runs — selects the inference route alias. */
export function CloudModelPicker({
  gatewayOrigin,
  bearerToken,
  value,
  onChange
}: {
  gatewayOrigin: string
  bearerToken?: string
  value?: string
  onChange: (alias: string | undefined) => void
}) {
  const { t } = useTranslation()
  const [models, setModels] = useState<CloudModelOption[]>([])

  useEffect(() => {
    if (!gatewayOrigin) return
    void fetchGatewayModels(gatewayOrigin, bearerToken).then(setModels)
  }, [gatewayOrigin, bearerToken])

  if (models.length === 0) return null

  return (
    <div className="flex items-center gap-2">
      <span aria-hidden className="text-[13px] text-[#3f3f3f]">
        {t('agents.cloudModel.label')}
      </span>
      <SearchSelect
        id="cloud-model"
        size="sm"
        searchable={models.length > 8}
        allowClear={false}
        aria-label={t('agents.cloudModel.label')}
        value={value ?? ''}
        options={[
          { value: '', label: t('agents.cloudModel.auto') },
          ...models.map((model) => ({ value: model.id, label: model.label }))
        ]}
        onChange={(next) => onChange(next || undefined)}
      />
    </div>
  )
}
