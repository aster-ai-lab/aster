import { ConfirmDialog } from '@aster-ai-lab/ui/components/composites/confirm-dialog'
import { Button } from '@aster-ai-lab/ui/components/primitives/button'
import { Input } from '@aster-ai-lab/ui/components/primitives/input'
import { Pagination, PaginationContent, PaginationItem } from '@aster-ai-lab/ui/components/primitives/pagination'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@aster-ai-lab/ui/components/primitives/select'
import { Input as Textarea } from '@aster-ai-lab/ui/components/primitives/textarea'
import i18next from 'i18next'
import { Box, Circle, Focus, Home, Move, Redo2, Rotate3d, Save, Scaling, Trash2, Undo2, X } from 'lucide-react'
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'

import { Host } from './host.mjs'
import { captureEditor, restoreEditor } from './editor-recovery.mjs'
import { Inspector } from './inspector.jsx'
import en from './locales/en-us.json'
import zh from './locales/zh-cn.json'
import { ProjectSession } from './project-session.mjs'
import { RecoveryJournal } from './recovery-journal.mjs'
import { primitiveEntity, SceneModel } from './scene-model.mjs'
import { SceneTree } from './scene-tree.jsx'
import { TwinViewport } from './viewport.mjs'

const host = new Host()
const model = new SceneModel()
const projects = new ProjectSession(host, model, () => recovery.checkpoint())
let recoveryUI = { workspaceId: '', projectId: '', title: '', homeTask: '', selected: null, inspector: null }
const recovery = new RecoveryJournal(host, () => captureEditor(model, projects, recoveryUI), (value) => {
  recoveryUI = restoreEditor(value, model, projects)
})
await i18next.init({
  lng: 'zh',
  fallbackLng: 'en',
  resources: { en: { translation: en }, zh: { translation: zh } },
  interpolation: { escapeValue: false }
})

function App() {
  const subscribe = useCallback((listener) => {
    model.addEventListener('change', listener)
    return () => model.removeEventListener('change', listener)
  }, [])
  const version = useSyncExternalStore(subscribe, () => model.version)
  const [, setLanguage] = useState(i18next.language)
  const t = i18next.t.bind(i18next)
  const [selected, setSelected] = useState(null)
  const [mode, setMode] = useState('translate')
  const [space, setSpace] = useState('world')
  const [panel, setPanel] = useState('')
  const [error, setError] = useState(null)
  const [busy, setBusy] = useState(null)
  const [initialized, setInitialized] = useState(false)
  const locked = useRef(false)
  const [workspaces, setWorkspaces] = useState([])
  const [workspaceId, setWorkspace] = useState('')
  const [projectId, setProject] = useState('')
  const [page, setPage] = useState({ items: [], total: 0, page: 1, limit: 20 })
  const [title, setTitle] = useState('')
  const [homeTask, setHomeTask] = useState('')
  const homeTaskRef = useRef('')
  const homeDelivery = useRef(null)
  const [confirmation, setConfirmation] = useState(null)
  const viewportElement = useRef(null)
  const viewport = useRef(null)
  const inspector = useRef(() => true)
  const retryInitialization = useRef(() => {})
  const choose = useRef(null)
  const summary = useRef(null)
  const entity = selected === null || !Object.hasOwn(model.scene.entities, selected) ? null : model.scene.entities[selected]
  const pending = !!projects.saveIntent || !!projects.createIntent
  const editingUnavailable = !!busy || !initialized
  const holdsRecovery = () => model.dirty || !!projects.saveIntent || !!projects.createIntent || !!homeTaskRef.current || !!recoveryUI.inspector

  const acceptHomeSeed = useRef(null)
  acceptHomeSeed.current = async () => {
    if (!recovery.ready || locked.current) return
    const message = host.homeSeed
    if (!message || !Number.isSafeInteger(message.delivery) || message.delivery <= 0) return
    host.homeSeed = null
    const content = message.seed?.content
    const holdsWork = model.dirty || pending || locked.current || !!homeTaskRef.current
    const accepted = !holdsWork && typeof content === 'string' && content.trim().length > 0 && content.length <= 24000
    if (accepted) {
      homeTaskRef.current = content
      setHomeTask(content)
      recoveryUI.homeTask = content
      try {
        await recovery.checkpoint()
      } catch {
        host.reportHome(message.delivery, false, undefined, true)
        failure('recovery_unavailable')
        return
      }
      homeDelivery.current = message
    }
    host.reportHome(message.delivery, accepted, accepted ? message.seed : undefined, holdsWork || accepted)
  }
  useEffect(() => {
    const receive = () => { void acceptHomeSeed.current() }
    host.addEventListener('home-seed', receive)
    receive()
    return () => host.removeEventListener('home-seed', receive)
  }, [])
  useEffect(() => {
    const message = homeDelivery.current
    if (message) host.reportHome(message.delivery, true, message.seed, model.dirty || pending || !!homeTask)
  }, [homeTask, version, pending])

  const failure = (value) => {
    const reason = typeof value === 'string' ? value : value.reason
    setError(i18next.exists(reason ?? '') ? reason : 'unavailable')
    queueMicrotask(() => summary.current?.focus())
  }
  const run = async (label, action) => {
    if (locked.current) return
    locked.current = true
    setBusy(label)
    setError(null)
    try {
      await action()
      return true
    } catch (value) {
      failure(value)
      return false
    } finally {
      locked.current = false
      setBusy(null)
      queueMicrotask(() => { void acceptHomeSeed.current() })
    }
  }
  const edit = (edits, revision) => {
    if (locked.current || !initialized) throw Object.assign(new Error('busy'), { reason: 'busy' })
    model.edit(edits, revision)
  }
  const safeEdit = (action) => {
    if (locked.current || !initialized || !inspector.current()) return
    try {
      action()
    } catch (value) {
      failure(value)
    }
  }
  choose.current = (id) => {
    if (locked.current || !initialized || !inspector.current()) return
    setSelected(id)
  }

  useEffect(() => {
    const environment = () => {
      const locale = host.environment?.locale ?? 'zh-CN'
      let language = 'en'
      try {
        language = new Intl.Locale(locale).language === 'zh' ? 'zh' : 'en'
      } catch {}
      void i18next.changeLanguage(language).then(() => {
        document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en-US'
        document.title = i18next.t('app_title')
        setLanguage(language)
      })
      if (!locked.current && !workspaces.length)
        void run('loading', async () => {
          if (!recovery.ready) {
            await recovery.load()
            setWorkspace(recoveryUI.workspaceId)
            setProject(recoveryUI.projectId)
            setTitle(recoveryUI.title)
            setHomeTask(recoveryUI.homeTask)
            homeTaskRef.current = recoveryUI.homeTask
            setSelected(recoveryUI.selected)
          }
          if (projects.scope && !projects.scope.lease) await projects.renew()
          setInitialized(true)
          const values = await host.request({ operation: 'workspaces' })
          setWorkspaces(values)
          if (!values.length) setError('no_workspaces')
          if (recoveryUI.workspaceId) await list(recoveryUI.workspaceId)
          if (projects.saveIntent || projects.createIntent) setError('outcome_unknown')
        })
    }
    retryInitialization.current = environment
    host.addEventListener('environment', environment)
    if (host.environment) environment()
    else if (!host.port) setError('closed')
    return () => host.removeEventListener('environment', environment)
  }, [])

  useEffect(() => {
    if (!initialized) return
    recoveryUI = { ...recoveryUI, workspaceId, projectId, title, homeTask, selected }
    if (recoveryUI.inspector?.id !== selected) recoveryUI.inspector = null
    void recovery.schedule().catch(() => failure('recovery_unavailable'))
    host.reportRecovery(holdsRecovery())
  }, [initialized, version, workspaceId, projectId, title, homeTask, selected, busy])

  useEffect(() => {
    const checkpoint = async ({ detail }) => {
      if (locked.current || !recovery.ready) { host.checkpointResult(detail.id, false, true); return }
      const renewing = detail.purpose === 'renew'
      if (renewing) { locked.current = true; setBusy('loading') }
      try {
        await recovery.checkpoint()
        host.checkpointResult(detail.id, true, holdsRecovery())
      } catch {
        if (renewing) { locked.current = false; setBusy(null) }
        host.checkpointResult(detail.id, false, true)
        failure('recovery_unavailable')
      }
    }
    host.addEventListener('checkpoint', checkpoint)
    return () => host.removeEventListener('checkpoint', checkpoint)
  }, [])

  useEffect(() => {
    try {
      viewport.current = new TwinViewport(viewportElement.current, {
        onSelect: (id) => choose.current(id),
        onTransform: (id, transform, revision) => {
          try {
            if (locked.current || !recovery.ready) throw new Error('busy')
            const current = model.scene.entities[id]
            if (!current) throw new Error('Missing selected object')
            model.edit([{ kind: 'replace', id, entity: { ...current, transform } }], revision)
          } catch (value) {
            failure(value)
            viewport.current?.update(model.scene)
          }
        },
        onError: failure
      })
      viewport.current.update(model.scene)
    } catch {
      failure('render_unavailable')
    }
    return () => {
      viewport.current?.dispose()
      viewport.current = null
    }
  }, [])
  useEffect(() => {
    viewport.current?.update(model.scene)
  }, [version])
  useEffect(() => {
    viewport.current?.select(selected)
  }, [selected, version])
  useEffect(() => {
    viewport.current?.mode(mode)
  }, [mode])
  useEffect(() => {
    viewport.current?.space(space)
  }, [space])
  useEffect(() => {
    if (viewport.current) viewport.current.transform.enabled = initialized && !busy
  }, [initialized, busy])

  const guard = (action) => {
    if (locked.current || !initialized || pending || !inspector.current()) return
    if (model.dirty) setConfirmation({ kind: 'discard', key: crypto.randomUUID(), action })
    else void action()
  }
  const list = async (id, number = 1) => {
    let result
    try {
      result = await host.request({
        operation: 'projects',
        workspaceId: id,
        page: number,
        limit: 20
      })
    } catch (value) {
      setPage({ items: [], total: 0, page: number, limit: 20 })
      throw value
    }
    setPage(result)
  }
  const changeWorkspace = (id) =>
    guard(() =>
      run('loading', async () => {
        if (projects.scope) await host.request({ operation: 'close', lease: projects.scope.lease }).catch(() => {})
        projects.scope = null
        model.reset()
        setSelected(null)
        setWorkspace(id)
        setProject('')
        setPage({ items: [], total: 0, page: 1, limit: 20 })
        await list(id)
      })
    )
  const open = (id) =>
    guard(() =>
      run('loading', async () => {
        await projects.open(workspaceId, id)
        setProject(id)
        setSelected(null)
        if (projects.scope.missing) setError('missing_scene')
      })
    )
  const save = () => {
    if (!initialized || !inspector.current()) return
    void run('saving', () => projects.save())
  }
  const create = () => {
    if (!initialized || !inspector.current()) return
    void run('saving', async () => {
      const result = await projects.create(workspaceId, title.trim())
      setTitle('')
      setProject(result.id)
      try {
        await projects.open(workspaceId, result.id)
        await list(workspaceId)
        if (projects.scope.missing) setError('missing_scene')
      } catch {
        if (projects.scope) await host.request({ operation: 'close', lease: projects.scope.lease }).catch(() => {})
        projects.scope = null
        setError('created_refresh_failed')
      }
    })
  }
  const add = (kind) =>
    safeEdit(() => {
      const id = crypto.randomUUID()
      const name = t(`${kind}_name`, { count: Object.keys(model.scene.entities).length + 1 })
      model.edit([{ kind: 'add', id, entity: primitiveEntity(name, kind) }])
      setSelected(id)
    })
  const remove = () =>
    safeEdit(() => {
      if (Object.values(model.scene.entities).some((value) => value.parent === selected)) {
        failure('has_children')
        return
      }
      const id = selected
      const revision = model.scene.revision
      setConfirmation({
        kind: 'delete',
        key: `${id}:${revision}`,
        name: entity.name,
        action: () => {
          model.edit([{ kind: 'remove', id }], revision)
          setSelected(null)
          viewportElement.current?.focus()
        }
      })
    })

  const commands = useRef(null)
  commands.current = {
    save,
    undo: () => safeEdit(() => model.undo()),
    redo: () => safeEdit(() => model.redo())
  }
  useEffect(() => {
    const key = (event) => {
      if (event.isComposing || event.defaultPrevented || !(event.ctrlKey || event.metaKey) || event.altKey) return
      if (event.key.toLowerCase() === 's') {
        event.preventDefault()
        commands.current.save()
        return
      }
      if (event.target.closest('input,textarea,[contenteditable="true"]') || event.key.toLowerCase() !== 'z') return
      event.preventDefault()
      commands.current[event.shiftKey ? 'redo' : 'undo']()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [])

  return (
    <main className="twin-app" aria-busy={!!busy}>
      <header className="twin-header">
        <h1>{t('app_title')}</h1>
        <Button
          size="icon-navbar"
          variant="ghost"
          aria-label={t('undo')}
          disabled={editingUnavailable || !model.past.length}
          onClick={() => safeEdit(() => model.undo())}>
          <Undo2 />
        </Button>
        <Button
          size="icon-navbar"
          variant="ghost"
          aria-label={t('redo')}
          disabled={editingUnavailable || !model.future.length}
          onClick={() => safeEdit(() => model.redo())}>
          <Redo2 />
        </Button>
        <Button
          variant="outline"
          disabled={editingUnavailable || !projects.scope || pending}
          onClick={() => open(projects.scope.projectId)}>
          {t('read_scene')}
        </Button>
        <Button
          loading={busy === 'saving'}
          disabled={
            editingUnavailable || !projects.scope || (projects.scope.missing && !projects.saveIntent) || !!projects.createIntent
          }
          onClick={save}>
          <Save />
          {t(projects.saveIntent ? 'retry_save' : 'save')}
        </Button>
      </header>
      {homeTask ? (
        <section className="flex shrink-0 items-start gap-3 border-b border-[#efefef] px-4 py-3" aria-label={t('scene_request')}>
          <div className="min-w-0 flex-1">
            <label htmlFor="scene-request" className="text-sm font-medium">{t('scene_request')}</label>
            <Textarea id="scene-request" className="mt-1 min-h-16" value={homeTask} onChange={(event) => {
              homeTaskRef.current = event.target.value
              recoveryUI.homeTask = event.target.value
              setHomeTask(event.target.value)
            }} />
            <p className="mt-1 text-xs text-[#3f3f3f]">{t('scene_request_preview')}</p>
          </div>
          <Button variant="ghost" size="icon-navbar" aria-label={t('clear_request')} onClick={() => {
            homeTaskRef.current = ''
            recoveryUI.homeTask = ''
            setHomeTask('')
          }}><X /></Button>
        </section>
      ) : null}
      <section className="twin-projects" aria-label={t('project')}>
        <label htmlFor="workspace-select">
          {t('workspace')}
          <Select
            value={workspaceId}
            disabled={editingUnavailable || pending || !workspaces.length}
            onValueChange={changeWorkspace}>
            <SelectTrigger id="workspace-select">
              <SelectValue placeholder={t('select_workspace')} />
            </SelectTrigger>
            <SelectContent>
              {workspaces.map((workspace) => (
                <SelectItem key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
        <label htmlFor="project-select">
          {t('project')}
          <Select
            value={page.items.some((item) => item.id === projectId) ? projectId : ''}
            disabled={editingUnavailable || pending || !workspaceId || !page.items.length}
            onValueChange={open}>
            <SelectTrigger id="project-select">
              <SelectValue placeholder={t('select_project')} />
            </SelectTrigger>
            <SelectContent>
              {page.items.map((project) => (
                <SelectItem key={project.id} value={project.id}>
                  {project.title}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
        {page.total > page.limit && (
          <Pagination aria-label={t('project')} className="w-auto mx-0">
            <PaginationContent>
              <PaginationItem>
                <Button
                  variant="ghost"
                  disabled={!!busy || pending || page.page <= 1}
                  onClick={() => void run('loading', () => list(workspaceId, page.page - 1))}>
                  {t('previous')}
                </Button>
              </PaginationItem>
              <PaginationItem>
                <span>{t('page', { page: page.page })}</span>
              </PaginationItem>
              <PaginationItem>
                <Button
                  variant="ghost"
                  disabled={!!busy || pending || page.page * page.limit >= page.total}
                  onClick={() => void run('loading', () => list(workspaceId, page.page + 1))}>
                  {t('next')}
                </Button>
              </PaginationItem>
            </PaginationContent>
          </Pagination>
        )}
        <form
          className="twin-create"
          onSubmit={(event) => event.preventDefault()}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.nativeEvent.isComposing && event.target.tagName === 'INPUT') {
              event.preventDefault()
              if (workspaceId && title.trim()) create()
            }
          }}>
          <label htmlFor="project-title">
            {t('project_name')}
            <Input
              id="project-title"
              maxLength={128}
              required
              value={title}
              disabled={editingUnavailable || pending}
              onChange={(event) => { recoveryUI.title = event.target.value; setTitle(event.target.value) }}
            />
          </label>
          <Button
            variant="outline"
            type="button"
            onClick={create}
            disabled={editingUnavailable || !workspaceId || !title.trim() || !!projects.saveIntent}>
            {t(projects.createIntent ? 'retry' : 'create_project')}
          </Button>
        </form>
        {!workspaces.length && host.port && (
          <Button
            variant="ghost"
            disabled={!!busy}
            onClick={() =>
              retryInitialization.current()
            }>
            {t('retry')}
          </Button>
        )}
      </section>
      {error && (
        <div className="twin-error" role="alert" tabIndex={-1} ref={summary}>
          {t(error)}
          {error === 'created_refresh_failed' && projectId && (
            <Button variant="outline" disabled={!!busy} onClick={() => open(projectId)}>
              {t('read_scene')}
            </Button>
          )}
          {projects.saveIntent && (
            <Button
              variant="outline"
              disabled={!!busy}
              onClick={() =>
                void run('loading', async () => {
                  try {
                    await projects.reconcile()
                  } catch (value) {
                    if (value.reason === 'not_found')
                      throw Object.assign(new Error('outcome_unknown'), { reason: 'outcome_unknown' })
                    throw value
                  }
                })
              }>
              {t('reconcile_save')}
            </Button>
          )}
          {error === 'missing_scene' && (
            <Button
              variant="outline"
              disabled={!!busy}
              onClick={() => {
                if (inspector.current()) void run('saving', () => projects.save({ create: true }))
              }}>
              {t('create_scene')}
            </Button>
          )}
          {error === 'conflict' && (
            <Button variant="outline" disabled={!!busy} onClick={() => open(projects.scope.projectId)}>
              {t('read_scene')}
            </Button>
          )}
          {!pending && workspaceId && (
            <Button
              variant="ghost"
              disabled={!!busy}
              onClick={() => void run('loading', () => list(workspaceId, page.page))}>
              {t('retry')}
            </Button>
          )}
        </div>
      )}
      <div className="twin-layout" data-panel={panel}>
        <SceneTree entities={model.scene.entities} selected={selected} onSelect={(id) => choose.current(id)} t={t} />
        <section className="twin-scene" aria-label={t('viewport')}>
          <div className="twin-toolbar" role="toolbar" aria-label={t('viewport')}>
            <Button className="twin-mobile-panel" variant="ghost" onClick={() => setPanel('scene')}>
              {t('scene_tree')}
            </Button>
            <Button className="twin-mobile-panel" variant="ghost" onClick={() => setPanel('properties')}>
              {t('properties')}
            </Button>
            <Button variant="ghost" aria-label={t('add_box')} disabled={editingUnavailable} onClick={() => add('box')}>
              <Box />
            </Button>
            <Button variant="ghost" aria-label={t('add_sphere')} disabled={editingUnavailable} onClick={() => add('sphere')}>
              <Circle />
            </Button>
            {[
              [Move, 'translate'],
              [Rotate3d, 'rotate'],
              [Scaling, 'scale']
            ].map(([Icon, value]) => (
              <Button
                key={value}
                variant="ghost"
                aria-label={t(value)}
                aria-pressed={mode === value}
                onClick={() => {
                  if (inspector.current()) setMode(value)
                }}>
                <Icon />
              </Button>
            ))}
            <Button variant="ghost" onClick={() => setSpace(space === 'world' ? 'local' : 'world')}>
              {t(space === 'world' ? 'world_space' : 'local_space')}
            </Button>
            <Button variant="ghost" aria-label={t('home_camera')} onClick={() => viewport.current?.home()}>
              <Home />
            </Button>
            <Button
              variant="ghost"
              aria-label={t('focus')}
              disabled={!entity}
              onClick={() => viewport.current?.focus()}>
              <Focus />
            </Button>
            <Button variant="ghost" aria-label={t('delete')} disabled={editingUnavailable || !entity} onClick={remove}>
              <Trash2 />
            </Button>
          </div>
          <div ref={viewportElement} className="twin-viewport" tabIndex={0} aria-label={t('viewport')} />
        </section>
        {entity ? (
          <Inspector
            key={selected}
            id={selected}
            entity={entity}
            revision={model.scene.revision}
            onEdit={edit}
            register={inspector}
            disabled={!!busy || !initialized}
            initialDraft={recoveryUI.inspector}
            onDraftChange={(draft) => {
              recoveryUI.inspector = draft
              host.reportRecovery(holdsRecovery())
              if (initialized) void recovery.schedule().catch(() => failure('recovery_unavailable'))
            }}
            t={t}
          />
        ) : (
          <aside className="twin-properties">
            <h2>{t('properties')}</h2>
            <p className="twin-empty">{t('select_object')}</p>
          </aside>
        )}
        {panel && (
          <Button
            className="twin-mobile-panel twin-close-panel"
            variant="outline"
            aria-label={t('close_panel')}
            onClick={() => setPanel('')}>
            <X />
          </Button>
        )}
      </div>
      <footer className="twin-status" role="status">
        <span>
          {t(busy ?? (model.dirty ? 'unsaved' : projects.scope && !projects.scope.missing ? 'saved' : 'local_draft'))}
        </span>
        <span>{projects.scope?.projectId ?? t('no_project')}</span>
      </footer>
      <ConfirmDialog
        open={!!confirmation}
        operationKey={confirmation?.key}
        onOpenChange={(value) => {
          if (!value) setConfirmation(null)
        }}
        title={t(confirmation?.kind === 'delete' ? 'delete_title' : 'discard_title')}
        description={t(confirmation?.kind === 'delete' ? 'delete_description' : 'discard_description', {
          name: confirmation?.name
        })}
        confirmText={t(confirmation?.kind === 'delete' ? 'remove' : 'discard')}
        cancelText={t('cancel')}
        failureText={t('unavailable')}
        destructive
        onConfirm={async () => {
          try {
            return await confirmation?.action()
          } catch (value) {
            failure(value)
            return false
          }
        }}
      />
    </main>
  )
}

createRoot(document.getElementById('app')).render(<App />)
