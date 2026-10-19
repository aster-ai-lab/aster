import { Combobox } from '@a3s-lab/ui'
import { useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { useRestingDialog } from '@renderer/components/restingDialog'
import {
  isHarnessAssembleId,
  readHarnessAssembleId,
  subscribeHarnessAssemble,
  writeHarnessAssembleId
} from '@renderer/services/harnessAssemble'
import { toast } from '@renderer/services/toast'

import { diffComponents } from './harnessDiff'
import {
  diffEvolution,
  readHarnessEvolution,
  subscribeHarnessEvolution,
  writeHarnessScenario
} from './harnessEvolution'
import { harnessSectionKey, type HarnessExecutionStep } from './harnessExecutionFlow'
import { HarnessFigure } from './HarnessFigure'
import { figureStep } from './harnessFigureLayout'
import { defaultDiffBase, HARNESS_SNAPSHOTS, snapshotById, type HarnessSnapshot } from './harnessSnapshots'

import './harnessDesignerHost.css'

const logger = loggerService.withContext('HarnessFlowDesigner')

/** Architecture figure for the assemble the next run reads. */
export function HarnessFlowDesigner() {
  const { t } = useTranslation()
  const [selectedId, setSelectedId] = useState<string>()
  const snapshotId = useSyncExternalStore(subscribeHarnessAssemble, readHarnessAssembleId, readHarnessAssembleId)
  const book = useSyncExternalStore(subscribeHarnessEvolution, readHarnessEvolution, readHarnessEvolution)
  const [diffOpen, setDiffOpen] = useState(false)
  const diffPresence = useRestingDialog(diffOpen)
  const [baseId, setBaseId] = useState(() => defaultDiffBase(readHarnessAssembleId()))
  const step = useMemo(() => figureStep(selectedId ?? ''), [selectedId])
  const snapshot = snapshotById(snapshotId)
  const base = snapshotById(baseId)
  const scenario = book[snapshot.id].scenario

  useEffect(() => {
    setSelectedId(undefined)
    setBaseId((current) => (current === snapshotId ? defaultDiffBase(snapshotId) : current))
  }, [snapshotId])

  const pickVersion = (id: HarnessSnapshotId) => {
    void writeHarnessAssembleId(id).catch((error: unknown) => {
      logger.error('Failed to save Harness recipe', error as Error)
      toast.error(t('common.save_failed'))
    })
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="harness-bar">
        <div className="harness-bar__lead">
          <p className="harness-bar__kicker" data-ui="settings.evolution.summary">
            {t('settings.evolution.summary')}
          </p>
          <label className="harness-bar__scene">
            <span>{t('settings.evolution.versions.scenario')}</span>
            <textarea
              className="harness-scenario"
              data-ui="settings.evolution.scenario"
              onChange={(event) => writeHarnessScenario(snapshot.id, event.target.value)}
              rows={2}
              value={scenario === null ? t(snapshot.scenarioKey) : scenario}
            />
          </label>
          <p className="harness-bar__note" data-ui="settings.evolution.version-note">
            {t(snapshot.noteKey)}
          </p>
          <p className="harness-bar__meta">
            <span data-ui="settings.evolution.versions.live">{t('settings.evolution.versions.live')}</span>
            <span data-ui="settings.evolution.versions.record">{t('settings.evolution.versions.record')}</span>
          </p>
          {step ? (
            <RoleLine step={step} />
          ) : (
            <p className="harness-bar__hint" data-ui="settings.evolution.hint">
              {t('settings.evolution.inspector.hint')}
            </p>
          )}
        </div>
        <div className="harness-bar__tools">
          {diffOpen ? (
            <VersionSelect
              id="harness-diff-base"
              label={t('settings.evolution.diff.base')}
              ui="settings.evolution.diff.base"
              value={base.id}
              onChange={(id) => setBaseId(id)}
            />
          ) : null}
          <VersionSelect
            id="harness-version"
            label={t('settings.evolution.versions.button')}
            live
            ui="settings.evolution.versions"
            value={snapshot.id}
            onChange={pickVersion}
          />
          <button
            aria-pressed={diffOpen}
            className="inline-flex h-8 shrink-0 items-center rounded-lg border border-[var(--color-border)] bg-white px-2.5 text-xs whitespace-nowrap text-[var(--color-ink)]"
            data-ui="settings.evolution.diff"
            onClick={() => setDiffOpen((open) => !open)}
            type="button">
            {diffOpen ? t('settings.evolution.diff.close') : t('settings.evolution.diff.open')}
          </button>
        </div>
      </header>
      <div className="relative flex min-h-0 flex-1 flex-col">
        {diffPresence.present ? (
          <DiffPanel base={base} book={book} state={diffPresence.state} target={snapshot} />
        ) : null}
        <HarnessFigure key={snapshot.id} onSelect={setSelectedId} snapshot={snapshot} />
      </div>
    </div>
  )
}

function VersionSelect({
  id,
  label,
  live,
  ui,
  value,
  onChange
}: {
  id: string
  label: string
  live?: boolean
  ui: string
  value: HarnessSnapshotId
  onChange: (id: HarnessSnapshotId) => void
}) {
  const { t } = useTranslation()
  const book = useSyncExternalStore(subscribeHarnessEvolution, readHarnessEvolution, readHarnessEvolution)
  const options = HARNESS_SNAPSHOTS.map((item) => ({
    value: item.id,
    label: t(item.titleKey),
    description: book[item.id].scenario?.trim() || t(item.scenarioKey)
  }))
  return (
    <div
      className="inline-flex items-center gap-1.5 text-xs text-[var(--color-ink-muted)]"
      data-assemble={live ? value : undefined}
      data-ui={ui}>
      <span id={id}>{label}</span>
      <Combobox
        aria-labelledby={id}
        className="harness-version-select bg-white text-xs shadow-none"
        emptyText={t('settings.evolution.versions.empty')}
        onChange={(next) => {
          const picked = Array.isArray(next) ? next[0] : next
          if (isHarnessAssembleId(picked)) onChange(picked)
        }}
        options={options}
        popoverAlign="end"
        searchPlaceholder={t('settings.evolution.versions.search')}
        searchable
        size="sm"
        value={value}
        width={220}
      />
    </div>
  )
}

function scenarioOf(
  book: ReturnType<typeof readHarnessEvolution>,
  snapshot: HarnessSnapshot,
  translate: (key: string) => string
): string {
  return book[snapshot.id].scenario?.trim() || translate(snapshot.scenarioKey)
}

function DiffPanel({
  base,
  book,
  state,
  target
}: {
  base: HarnessSnapshot
  book: ReturnType<typeof readHarnessEvolution>
  state: 'open' | 'closed'
  target: HarnessSnapshot
}) {
  const { t } = useTranslation()
  const composition = diffEvolution(book[base.id], book[target.id], {
    base: scenarioOf(book, base, t),
    next: scenarioOf(book, target, t)
  })
  const rows = diffComponents(base.components, target.components)
  const moved = rows.some((row) => row.kind !== 'same')
  return (
    <aside
      aria-label={t('settings.evolution.diff.title')}
      className={`harness-diff a3s-dialog-surface${state === 'closed' ? ' pointer-events-none' : ''}`}
      data-state={state}
      data-ui="settings.evolution.diff.panel"
      role="dialog">
      <h2>{t('settings.evolution.diff.title')}</h2>
      <p className="harness-diff__pair">
        {t(base.titleKey)} → {t(target.titleKey)}
      </p>
      <p data-ui="settings.evolution.diff.note">{scenarioOf(book, target, t)}</p>
      <h3>{t('settings.evolution.diff.tools')}</h3>
      {composition.length > 0 ? (
        <ol>
          {composition.map((row) => (
            <li
              data-asset={row.asset === 'scenario' ? 'scenario' : `${row.asset}:${row.name}`}
              data-diff={row.kind}
              key={`${row.asset}:${row.kind}:${row.name}`}>
              <span>{t(`settings.evolution.diff.${row.kind}`)}</span>
              {row.asset === 'scenario' ? t('settings.evolution.diff.scenario') : row.name}
            </li>
          ))}
        </ol>
      ) : (
        <p>{t('settings.evolution.diff.toolsEmpty')}</p>
      )}
      <h3>{t('settings.evolution.diff.posture')}</h3>
      {moved ? (
        <ol>
          {rows.map((row) => (
            <li data-component={row.text} data-diff={row.kind} key={`${row.kind}:${row.text}`}>
              <span>{t(`settings.evolution.diff.${row.kind === 'same' ? 'same' : row.kind}`)}</span>
              {row.text}
            </li>
          ))}
        </ol>
      ) : (
        <p>{t('settings.evolution.diff.empty')}</p>
      )}
      <p className="harness-diff__kernel">{t('settings.evolution.diff.kernel')}</p>
    </aside>
  )
}

type HarnessSnapshotId = (typeof HARNESS_SNAPSHOTS)[number]['id']

function RoleLine({ step }: { step: HarnessExecutionStep }) {
  const { t } = useTranslation()
  return (
    <p data-ui="settings.evolution.role">
      {t(harnessSectionKey(step))} · {t('settings.evolution.inspector.label')} {step.label} {t(step.roleKey)}
    </p>
  )
}
