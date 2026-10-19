import { useState } from 'react'
import { useTranslation } from 'react-i18next'

/** Result of a skill source commit. */
type CommitResult = { commitSha: string } | { error: string }

/**
 * "Publish to Cloud" button for a local agent's skill markdown.
 * Calls the member-plane skill-source commit endpoint.
 */
export function SkillPublishButton({
  assetId,
  markdown,
  message,
  onPublished,
  disabled
}: {
  assetId: string
  markdown: string
  message?: string
  onPublished?: (commitSha: string) => void
  disabled?: boolean
}) {
  const { t } = useTranslation()
  const [state, setState] = useState<'idle' | 'publishing' | 'done' | 'error'>('idle')
  const [result, setResult] = useState<CommitResult | null>(null)

  const publish = async () => {
    if (!assetId || !markdown.trim() || state === 'publishing') return
    setState('publishing')
    setResult(null)
    try {
      const origin = window.location?.origin ?? ''
      const response = await fetch(
        `${origin}/api/skills/${assetId}/skill-source`,
        {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ markdown, message: message ?? undefined })
        }
      )
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as { message?: string } | null
        setResult({ error: body?.message ?? `HTTP ${response.status}` })
        setState('error')
        return
      }
      const body = (await response.json()) as { data?: { commitSha?: string } }
      const sha = body.data?.commitSha ?? ''
      setResult({ commitSha: sha })
      setState('done')
      onPublished?.(sha)
    } catch (error) {
      setResult({ error: String(error) })
      setState('error')
    }
  }

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={() => void publish()}
        disabled={disabled || state === 'publishing' || !markdown.trim()}
        className="rounded-md border px-3 py-1.5 text-sm font-medium transition-colors hover:bg-accent disabled:opacity-50"
      >
        {state === 'publishing'
          ? t('agents.skillPublish.publishing')
          : state === 'done'
            ? t('agents.skillPublish.published')
            : t('agents.skillPublish.publish')}
      </button>
      {state === 'done' && result && 'commitSha' in result && (
        <span className="font-mono text-xs text-muted-foreground">
          {result.commitSha.slice(0, 12)}…
        </span>
      )}
      {state === 'error' && result && 'error' in result && (
        <span className="text-xs text-red-600 dark:text-red-400">{result.error}</span>
      )}
    </div>
  )
}
