import { useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import {
  COMPACTION_SYSTEM_PROMPT,
  DEFAULT_SPEC,
  INTENT_STAMP_MARKER,
  PRUNED_MARKER,
  SKILL_FIELDS,
  TRUNCATED_MARKER,
  admittedTool,
  internalsFocus,
  systemLine,
  type ToolGate
} from './harnessDetailCatalog'
import {
  HOST_SKILL_ID,
  addHarnessSkill,
  readHarnessEvolution,
  removeHarnessSkill,
  subscribeHarnessEvolution,
  writeHarnessSkill,
  writeHarnessTool
} from './harnessEvolution'
import { HARNESS_SNAPSHOTS, type HarnessSnapshot } from './harnessSnapshots'

const GATE_KEY: Record<ToolGate, string> = {
  always: 'settings.evolution.detail.gate.always',
  read: 'settings.evolution.detail.gate.read',
  write: 'settings.evolution.detail.gate.write',
  writeLocal: 'settings.evolution.detail.gate.writeLocal',
  readWrite: 'settings.evolution.detail.gate.readWrite',
  exec: 'settings.evolution.detail.gate.exec',
  search: 'settings.evolution.detail.gate.search',
  git: 'settings.evolution.detail.gate.git',
  session: 'settings.evolution.detail.gate.session',
  llm: 'settings.evolution.detail.gate.llm'
}

/** Prompt, tool, and skill breakdown for the clicked figure module. */
export function HarnessInternals({ detailId, snapshotId }: { detailId: string; snapshotId: HarnessSnapshot['id'] }) {
  const { t } = useTranslation()
  const book = useSyncExternalStore(subscribeHarnessEvolution, readHarnessEvolution, readHarnessEvolution)
  const edition = book[snapshotId]
  const focus = internalsFocus(detailId)
  const compactMounted = snapshotId !== 'lean'

  return (
    <div className="harness-figure__internals" data-focus={focus} data-ui="settings.evolution.detail.internals">
      <h3>{t('settings.evolution.detail.internals')}</h3>
      {focus === 'system' || focus === 'marker' ? (
        <section>
          <h4>{t('settings.evolution.detail.prompts')}</h4>
          <p>
            {t(
              focus === 'marker'
                ? 'settings.evolution.detail.marker'
                : snapshotId === 'host'
                  ? 'settings.evolution.detail.promptHost'
                  : 'settings.evolution.detail.promptStock'
            )}
          </p>
          <ul>
            {HARNESS_SNAPSHOTS.map((item) => {
              const line = systemLine(item.id)
              return (
                <li
                  data-admits={line ? 'true' : 'false'}
                  data-current={item.id === snapshotId ? 'true' : 'false'}
                  data-snapshot={item.id}
                  key={item.id}>
                  <span>{t(item.titleKey)}</span>
                  {line ? <code data-prompt={INTENT_STAMP_MARKER}>{line}</code> : null}
                </li>
              )
            })}
          </ul>
        </section>
      ) : null}
      {focus === 'compact' ? (
        <section>
          <h4>{t('settings.evolution.detail.prompts')}</h4>
          <p>
            {t(compactMounted ? 'settings.evolution.detail.compactMounted' : 'settings.evolution.detail.compactAbsent')}
          </p>
          <pre data-admitted={compactMounted ? 'slot' : 'engine'} data-ui="settings.evolution.detail.prompt">
            {COMPACTION_SYSTEM_PROMPT}
          </pre>
          <p data-marker="pruned">{PRUNED_MARKER}</p>
          <p data-marker="truncated">{TRUNCATED_MARKER.trim()}</p>
        </section>
      ) : null}
      {focus === 'tools' ? (
        <>
          <section>
            <h4>{t('settings.evolution.detail.tools')}</h4>
            <p>{t('settings.evolution.detail.composition')}</p>
            <p>{t('settings.evolution.detail.toolNote')}</p>
            <ol>
              {edition.tools.map((item) => {
                const tool = admittedTool(item.name)
                if (!tool) return null
                return (
                  <li data-included={item.included ? 'true' : 'false'} data-tool={tool.name} key={tool.name}>
                    <div className="harness-tool-head">
                      <input
                        aria-label={`${t('settings.evolution.detail.included')} ${tool.name}`}
                        checked={item.included}
                        onChange={(event) =>
                          writeHarnessTool(snapshotId, tool.name, { included: event.target.checked })
                        }
                        type="checkbox"
                      />
                      <strong>{tool.name}</strong>
                      <span>{t(GATE_KEY[tool.gate])}</span>
                    </div>
                    <textarea
                      aria-label={`${t('settings.evolution.detail.description')} ${tool.name}`}
                      onChange={(event) => writeHarnessTool(snapshotId, tool.name, { description: event.target.value })}
                      rows={2}
                      value={item.description.trim() || tool.description}
                    />
                    {tool.alternate ? <p>{tool.alternate}</p> : null}
                    <ul>
                      {tool.fields.map((field) => (
                        <li
                          data-one-of={field.oneOf}
                          data-param={field.name}
                          data-required={field.required ? 'true' : 'false'}
                          key={field.name}>
                          <code>{field.name}</code>
                          {field.required
                            ? t('settings.evolution.detail.required')
                            : field.oneOf
                              ? t('settings.evolution.detail.oneOf')
                              : t('settings.evolution.detail.optional')}
                        </li>
                      ))}
                    </ul>
                  </li>
                )
              })}
            </ol>
          </section>
          <section>
            <h4>{t('settings.evolution.detail.skills')}</h4>
            {edition.skills.length === 0 ? (
              <p data-ui="settings.evolution.detail.skills">{t('settings.evolution.detail.emptySkills')}</p>
            ) : (
              edition.skills.map((skill) => (
                <article className="harness-figure__skill" data-skill={skill.id} key={skill.id}>
                  <label>
                    <span>{t('settings.evolution.detail.skillName')}</span>
                    <input
                      data-skill-input="name"
                      onChange={(event) => writeHarnessSkill(snapshotId, skill.id, { name: event.target.value })}
                      type="text"
                      value={skill.name}
                    />
                  </label>
                  <label>
                    <span>{t('settings.evolution.detail.description')}</span>
                    <input
                      data-skill-input="description"
                      onChange={(event) => writeHarnessSkill(snapshotId, skill.id, { description: event.target.value })}
                      type="text"
                      value={
                        skill.description.trim() ||
                        (skill.id === HOST_SKILL_ID ? t('settings.evolution.versions.host.skill.description') : '')
                      }
                    />
                  </label>
                  <label>
                    <span>{t('settings.evolution.detail.skillTools')}</span>
                    <input
                      data-skill-input="allowed-tools"
                      onChange={(event) =>
                        writeHarnessSkill(snapshotId, skill.id, { allowedTools: event.target.value })
                      }
                      type="text"
                      value={skill.allowedTools}
                    />
                  </label>
                  <label>
                    <span>{t('settings.evolution.detail.skillBody')}</span>
                    <textarea
                      data-skill-input="content"
                      onChange={(event) => writeHarnessSkill(snapshotId, skill.id, { content: event.target.value })}
                      rows={3}
                      value={
                        skill.content.trim() ||
                        (skill.id === HOST_SKILL_ID ? t('settings.evolution.versions.host.skill.content') : '')
                      }
                    />
                  </label>
                  <button
                    className="harness-figure__mini"
                    onClick={() => removeHarnessSkill(snapshotId, skill.id)}
                    type="button">
                    {t('settings.evolution.detail.skillDelete')}
                  </button>
                </article>
              ))
            )}
            <button
              className="harness-figure__mini"
              data-ui="settings.evolution.detail.skill-add"
              onClick={() => addHarnessSkill(snapshotId)}
              type="button">
              {t('settings.evolution.detail.skillAdd')}
            </button>
            <p>{t('settings.evolution.detail.skillShape')}</p>
            <ul>
              {SKILL_FIELDS.map((name) => (
                <li data-skill-field={name} key={name}>
                  <code>{name}</code>
                </li>
              ))}
            </ul>
          </section>
        </>
      ) : null}
      {focus === 'budget' ? (
        <section>
          <h4>{t('settings.evolution.detail.fields')}</h4>
          <p>{t('settings.evolution.detail.same')}</p>
          <p>{t('settings.evolution.detail.budget')}</p>
          <dl>
            <div data-field="tool_budget">
              <dt>tool_budget</dt>
              <dd>{DEFAULT_SPEC.budget}</dd>
            </div>
          </dl>
        </section>
      ) : null}
      {focus === 'infer' ? (
        <section>
          <h4>{t('settings.evolution.detail.fields')}</h4>
          <p>{t('settings.evolution.detail.same')}</p>
          <p>{t('settings.evolution.detail.infer')}</p>
          <dl>
            <div data-field="model_attempts">
              <dt>model_attempts</dt>
              <dd>{DEFAULT_SPEC.modelAttempts}</dd>
            </div>
            <div data-field="step_limit">
              <dt>step_limit</dt>
              <dd>{DEFAULT_SPEC.stepLimit}</dd>
            </div>
            <div data-field="completion">
              <dt>CompletionRequest</dt>
              <dd>system, tools, summary, messages</dd>
            </div>
          </dl>
        </section>
      ) : null}
      {focus === 'permission' ? (
        <section>
          <h4>{t('settings.evolution.detail.fields')}</h4>
          <p>{t('settings.evolution.detail.noExtra')}</p>
          <dl>
            <div data-field="permission_overlay">
              <dt>permission_overlay</dt>
              <dd>true</dd>
            </div>
          </dl>
        </section>
      ) : null}
      {focus === 'gate' ? (
        <section>
          <h4>{t('settings.evolution.detail.fields')}</h4>
          <p>{t('settings.evolution.detail.noExtra')}</p>
          <dl>
            <div data-field="completion_gate">
              <dt>completion_gate</dt>
              <dd>true</dd>
            </div>
          </dl>
        </section>
      ) : null}
      {focus === 'none' ? <p>{t('settings.evolution.detail.noExtra')}</p> : null}
    </div>
  )
}
