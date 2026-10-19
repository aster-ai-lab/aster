import { Button, FieldGroup } from '@a3s-lab/ui'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import ToastHost from '@renderer/components/ToastHost'
import { WorkInboxWindow } from '@renderer/components/workInbox/WorkInboxWindow'
import { MemberOrgError, PERSONAL_CENTER, readCenterId } from '@renderer/data/memberOrgs'
import {
  createWorkItem,
  loadApprovers,
  loadTravelLinks,
  loadWorkCenter,
  notifyWorkInboxChanged,
  previewWorkProcess,
  startWorkProcess,
  type WorkCreateKind
} from '@renderer/workInbox/client'
import { type AppletPermission, readAppletPermissions } from '@shared/appletRuntime'

import { AppletPermissionDialog } from './AppletPermissionDialog'
import { allowPermissionGrant, entryPermissions, permissionGrantMatches } from './appletPermissionGrant'
import { AppletWindowBar } from './AppletWindowBar'
import { fieldsFromPackage, loadAppletPackage, type AppletChannel, type PinnedAppletPackage } from './loadAppletPackage'
import { loadAppletProfile, type AppletProfile } from './loadAppletProfile'
import { loadAppletSession } from './loadAppletSession'
import {
  emptyDrafts,
  officeFields,
  officeReady,
  type FormCheckpoint,
  type OfficeDrafts,
  type OfficeFormId
} from './officeAppletDraft'
import { NoticeIcon, OfficeAppletFields } from './OfficeAppletFields'
import { pdfHandoffUsesShell, takePdfToolHandoff, type PdfToolHandoff } from './pdfToolHandoff'
import { PlanEditorWindow } from './PlanEditorWindow'
import { submitApplet } from './submitApplet'
import {
  appletGlyph,
  INBOX_APPLET_ID,
  PDF_TOOLS_APPLET_ID,
  PLAN_APPLET_ID,
  type WorkbenchWindowAppletId
} from './WorkbenchApplets'
import { runAppletWindowAction } from './workbenchAppletWindowBridge'
import { WorkbenchPdfToolsWindow } from './WorkbenchPdfToolsDialog'

import './workbenchComposer.css'

type WorkbenchAppletWindowProps = {
  appletId: WorkbenchWindowAppletId
  channel?: AppletChannel | 'invalid'
  tool?: string | null
}

const OFFICE_KINDS = ['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'] as const

function officeKind(id: string): WorkCreateKind | null {
  return (OFFICE_KINDS as readonly string[]).includes(id) ? (id as WorkCreateKind) : null
}

/** Engine windows preview, then start. They do not open a single-step item. */
function processKeyFor(id: string): string | null {
  if (id === 'leave') return 'leave'
  if (id === 'expense') return 'expense'
  if (id === 'travel') return 'travel'
  if (id === 'clock') return 'clock'
  if (id === 'correction') return 'attendance_correction'
  if (id === 'probation') return 'probation'
  if (id === 'ai-resource') return 'ai_demo_resource'
  return null
}

type ProcessRoute =
  | { kind: 'idle' }
  | { kind: 'loading'; snapshot: string }
  | { kind: 'ok'; snapshot: string; nodes: { key: string; name: string }[] }
  | { kind: 'refused'; snapshot: string; message: string }

const PREVIEW_DEBOUNCE_MS = 200

type Phase = 'loading' | 'form' | 'failed' | 'signed_out'

type ProfileGrant = 'unknown' | 'ask' | 'allowed' | 'denied'

/**
 * Full-window body of a native applet window. The title drags the system
 * window. Window controls sit on the trailing edge as one neutral set on
 * every platform: minimize, zoom, close.
 */
export function WorkbenchAppletWindow({ appletId, channel = 'release', tool = null }: WorkbenchAppletWindowProps) {
  if (appletId === INBOX_APPLET_ID) {
    return (
      <>
        <WorkInboxWindow />
        <ToastHost />
      </>
    )
  }
  if (appletId === PDF_TOOLS_APPLET_ID) {
    return <PdfToolsApplet tool={tool} />
  }
  if (appletId === PLAN_APPLET_ID) {
    return (
      <>
        <PlanEditorWindow />
        <ToastHost />
      </>
    )
  }
  return <OfficeAppletWindow appletId={appletId} channel={channel} />
}

function PdfToolsApplet({ tool }: { tool: string | null }) {
  const { t } = useTranslation()
  const [shell] = useState(() => pdfHandoffUsesShell())
  const [ready, setReady] = useState(!shell)
  const [handoff, setHandoff] = useState<PdfToolHandoff | null>(null)
  useEffect(() => {
    if (!shell) return
    let live = true
    void takePdfToolHandoff().then((job) => {
      if (!live) return
      setHandoff(job)
      setReady(true)
    })
    return () => {
      live = false
    }
  }, [shell])
  if (!ready) {
    return (
      <section
        aria-label={t('workbench.feature.pdf.tools.title')}
        data-ui="workbench.applet.window"
        data-applet="pdf"
        data-presentation="window"
        className="fixed inset-0 bg-white"
      />
    )
  }
  return (
    <>
      <WorkbenchPdfToolsWindow initialKind={handoff?.kind ?? tool} initialFiles={handoff?.files ?? null} />
      <ToastHost />
    </>
  )
}

function OfficeAppletWindow({
  appletId,
  channel = 'release'
}: {
  appletId: OfficeFormId
  channel?: AppletChannel | 'invalid'
}) {
  const { t } = useTranslation()
  const Icon = appletId === 'notice' ? NoticeIcon : appletGlyph(appletId)
  const title = t(`workbench.applet.${appletId}`)
  const [attempt, setAttempt] = useState(0)
  const [phase, setPhase] = useState<Phase>('loading')
  const [pinned, setPinned] = useState<PinnedAppletPackage | null>(null)
  const [drafts, setDrafts] = useState<OfficeDrafts>(emptyDrafts)
  const [submitting, setSubmitting] = useState(false)
  const [submitFailed, setSubmitFailed] = useState(false)
  const [submitted, setSubmitted] = useState(false)
  const [submittedItemId, setSubmittedItemId] = useState('')
  const [routed, setRouted] = useState(false)
  const [past, setPast] = useState<FormCheckpoint[]>([])
  const [future, setFuture] = useState<FormCheckpoint[]>([])
  const [updateReady, setUpdateReady] = useState(false)
  const [sessionName, setSessionName] = useState('')
  const [grant, setGrant] = useState<ProfileGrant>('unknown')
  const [required, setRequired] = useState<AppletPermission[]>([])
  const [profile, setProfile] = useState<AppletProfile | null>(null)
  const [approvers, setApprovers] = useState<{ id: string; name: string }[]>([])
  const [approverGate, setApproverGate] = useState<'unknown' | 'personal' | 'ready' | 'empty'>('unknown')
  const [teamOrgId, setTeamOrgId] = useState('')
  const [route, setRoute] = useState<ProcessRoute>({ kind: 'idle' })
  const [submitError, setSubmitError] = useState('')
  const [travelLinks, setTravelLinks] = useState<{ id: string; title: string }[]>([])
  const idempotencyKey = useRef<string | null>(null)
  const snapshotRef = useRef<FormCheckpoint>({
    drafts: emptyDrafts(),
    submitted: false,
    submitFailed: false,
    routed: false
  })
  const processKey = processKeyFor(appletId)
  const fieldValues = officeFields(appletId, drafts)
  const fieldSnapshot = fieldValues ? JSON.stringify(fieldValues) : ''
  const fieldsRef = useRef(fieldValues)
  fieldsRef.current = fieldValues
  const teamProcess = processKey !== null && approverGate === 'ready'
  const routeReady = route.kind === 'ok' && route.snapshot === fieldSnapshot
  const ready =
    officeReady(appletId, drafts) &&
    approverGate !== 'unknown' &&
    approverGate !== 'empty' &&
    (!teamProcess || routeReady)
  snapshotRef.current = { drafts, submitted, submitFailed, routed }
  const identity =
    required.includes('user') && grant === 'allowed'
      ? { displayName: profile?.displayName || sessionName, avatar: profile?.avatar ?? '' }
      : null

  useEffect(() => {
    if (appletId === 'notice') {
      setApproverGate('personal')
      setTeamOrgId('')
      return
    }
    if (phase !== 'form' || grant !== 'allowed') return
    let cancel = false
    setApproverGate('unknown')
    setTeamOrgId('')
    void loadWorkCenter()
      .catch(() => null)
      .then(async (remote) => {
        const orgId = remote === null ? readCenterId() : remote
        const team = Boolean(orgId && orgId !== PERSONAL_CENTER)
        if (!team) {
          if (!cancel) {
            setTeamOrgId('')
            setApproverGate('personal')
          }
          return
        }
        try {
          const names = await loadApprovers()
          if (cancel) return
          setApprovers(names)
          setTeamOrgId(names.length > 0 ? orgId : '')
          setApproverGate(names.length > 0 ? 'ready' : 'empty')
          if (appletId === 'expense') {
            const links = await loadTravelLinks().catch(() => [])
            if (!cancel) setTravelLinks(links.map((item) => ({ id: item.id, title: item.title })))
          }
        } catch {
          if (!cancel) {
            setApprovers([])
            setTeamOrgId('')
            setApproverGate('empty')
          }
        }
      })
    return () => {
      cancel = true
    }
  }, [appletId, grant, phase])

  useEffect(() => {
    if (
      phase !== 'form' ||
      grant !== 'allowed' ||
      !processKey ||
      approverGate !== 'ready' ||
      !teamOrgId ||
      !fieldSnapshot
    ) {
      setRoute((current) => (current.kind === 'idle' ? current : { kind: 'idle' }))
      return
    }
    const snapshot = fieldSnapshot
    const values = fieldsRef.current
    if (!values) return
    let cancel = false
    setRoute({ kind: 'loading', snapshot })
    const timer = window.setTimeout(() => {
      void previewWorkProcess({
        orgId: teamOrgId,
        processKey,
        amount: values.amount ?? '',
        form: values
      }).then(
        (preview) => {
          if (cancel) return
          // Clock records a punch and has no approval nodes. Any other empty route is a refusal.
          if (processKey !== 'clock' && preview.nodes.length === 0) {
            setRoute({ kind: 'refused', snapshot, message: '' })
            return
          }
          setRoute({
            kind: 'ok',
            snapshot,
            nodes: preview.nodes.map((node) => ({ key: node.key, name: node.name }))
          })
        },
        (error: unknown) => {
          if (cancel) return
          setRoute({
            kind: 'refused',
            snapshot,
            message: error instanceof MemberOrgError ? error.message : ''
          })
        }
      )
    }, PREVIEW_DEBOUNCE_MS)
    return () => {
      cancel = true
      window.clearTimeout(timer)
    }
  }, [phase, grant, processKey, approverGate, teamOrgId, fieldSnapshot])

  useEffect(() => {
    document.getElementById('spinner')?.remove()
    const previous = document.title
    document.title = title
    if (channel === 'invalid') {
      setPhase('failed')
      setPinned(null)
      return () => {
        document.title = previous
      }
    }
    let current = true
    setPhase('loading')
    setPinned(null)
    setUpdateReady(false)
    setSubmitted(false)
    setSubmittedItemId('')
    setSubmitFailed(false)
    setSubmitError('')
    setSubmitting(false)
    setDrafts(emptyDrafts())
    setRouted(false)
    setPast([])
    setFuture([])
    setSessionName('')
    setGrant('unknown')
    setProfile(null)
    idempotencyKey.current = null
    void Promise.all([
      loadAppletSession(appletId),
      loadAppletPackage(appletId, globalThis.fetch, 1000, localStorage, channel)
    ]).then(
      ([session, loaded]) => {
        if (!current) return
        const nextFields = loaded ? fieldsFromPackage(loaded.body, appletId) : null
        if (session.status === 'signed_out') {
          setPhase('signed_out')
          return
        }
        if (session.status !== 'ready' || !loaded || !nextFields) {
          setPhase('failed')
          return
        }
        setPinned(loaded)
        setSessionName(session.displayName)
        const nextRequired = entryPermissions(readAppletPermissions())
        setRequired(nextRequired)
        setGrant(permissionGrantMatches(appletId, nextRequired) ? 'allowed' : 'ask')
        if (idempotencyKey.current == null) idempotencyKey.current = crypto.randomUUID()
        setPhase('form')
      },
      () => {
        if (current) setPhase('failed')
      }
    )
    return () => {
      current = false
      document.title = previous
    }
  }, [appletId, channel, title, attempt])

  useEffect(() => {
    if (!pinned) return
    let current = true
    void pinned.updateReady.then((ready) => {
      if (current && ready) setUpdateReady(true)
    })
    return () => {
      current = false
    }
  }, [pinned])

  useEffect(() => {
    if (!required.includes('user') || grant !== 'allowed') return
    let current = true
    void loadAppletProfile(appletId).then((loaded) => {
      if (current && loaded) setProfile(loaded)
    })
    return () => {
      current = false
    }
  }, [appletId, grant, required])

  const reload = () => setAttempt((value) => value + 1)

  const restore = (shot: FormCheckpoint) => {
    setDrafts(shot.drafts)
    setSubmitted(shot.submitted)
    setSubmitFailed(shot.submitFailed)
    setRouted(shot.routed)
  }

  const back = () => {
    if (past.length === 0) return
    const previous = past[past.length - 1]
    setFuture((ahead) => [snapshotRef.current, ...ahead])
    setPast((items) => items.slice(0, -1))
    restore(previous)
  }

  const forward = () => {
    if (future.length === 0) return
    const [next, ...rest] = future
    setPast((items) => [...items, snapshotRef.current])
    setFuture(rest)
    restore(next)
  }

  const rememberSubmit = (team: boolean) => {
    setPast((items) => [...items, snapshotRef.current])
    setFuture([])
    setRouted(team)
    setSubmitted(true)
    setSubmitFailed(false)
    setSubmitError('')
  }

  return (
    <section
      aria-label={title}
      data-ui="workbench.applet.window"
      data-applet={appletId}
      data-applet-version={pinned?.version}
      data-applet-source={pinned?.source}
      data-presentation="window"
      className="fixed inset-0 flex flex-col overflow-hidden bg-white text-[#171717]">
      <AppletWindowBar
        title={title}
        icon={<Icon />}
        canBack={past.length > 0}
        canForward={future.length > 0}
        onBack={back}
        onForward={forward}
        onHome={reload}
        onRefresh={reload}
      />
      {updateReady ? (
        <p
          data-ui="workbench.applet.update"
          className="m-0 border-b border-[#efefef] px-3 py-1.5 text-center text-[12px] leading-4 text-[#8a8a8a]">
          {t('workbench.applet.update_ready')}
        </p>
      ) : null}
      <div className="relative z-0 flex min-h-0 flex-1 flex-col overflow-hidden">
        {phase === 'form' && pinned && grant === 'allowed' && appletId !== 'approval' && identity?.displayName ? (
          <div className="flex shrink-0 justify-end px-4 pt-3 pb-1">
            <div
              data-ui={appletId === 'leave' ? 'workbench.applet.leave.profile' : 'workbench.applet.profile'}
              className="flex max-w-[70%] items-center gap-2">
              <p className="m-0 min-w-0 truncate text-right text-[13px] leading-5 font-medium text-[#171717]">
                {identity.displayName}
              </p>
              <LeavePortrait name={identity.displayName} avatar={identity.avatar} />
            </div>
          </div>
        ) : null}
        <div
          data-ui="workbench.applet.body"
          className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain">
          {phase === 'loading' ? (
            <div data-ui="workbench.applet.loading" className="flex h-full flex-col items-center justify-center gap-4">
              <span className="size-14">
                <Icon />
              </span>
              <p className="m-0 text-[13px] text-[#8a8a8a]">{t('workbench.applet.loading')}</p>
              <div className="h-1 w-[120px] overflow-hidden rounded-[8px] bg-[#f5f5f5]" aria-hidden>
                <div className="workbench-applet-load-bar h-full w-10 rounded-[8px] bg-[#171717]" />
              </div>
            </div>
          ) : phase === 'form' && pinned && grant === 'allowed' ? (
            <form
              data-ui="workbench.applet.form"
              className={`mx-auto flex w-full max-w-[640px] min-w-0 flex-col px-4 pb-6 sm:px-6 ${appletId === 'approval' ? 'pt-6' : 'pt-2'}`}
              onSubmit={(event) => {
                event.preventDefault()
                if (submitting || submitted || !idempotencyKey.current) return
                const key = idempotencyKey.current
                const values = officeFields(appletId, drafts)
                if (!values) return
                const startedKey = processKeyFor(appletId)
                if (
                  startedKey &&
                  approverGate === 'ready' &&
                  (route.kind !== 'ok' || route.snapshot !== JSON.stringify(values))
                ) {
                  return
                }
                setSubmitting(true)
                setSubmitFailed(false)
                setSubmitError('')
                const noteFailure = (error: unknown) => {
                  setSubmitting(false)
                  setSubmitFailed(true)
                  setSubmitError(error instanceof MemberOrgError ? error.message : '')
                }
                void (async () => {
                  const remote = await loadWorkCenter().catch(() => null)
                  const orgId = remote === null ? readCenterId() : remote
                  const team = Boolean(orgId && orgId !== PERSONAL_CENTER)
                  if (team && orgId && startedKey) {
                    try {
                      const item = await startWorkProcess({
                        orgId,
                        idempotencyKey: key,
                        processKey: startedKey,
                        amount: values.amount ?? '',
                        form: values
                      })
                      notifyWorkInboxChanged()
                      setSubmittedItemId(item.id)
                      setSubmitting(false)
                      rememberSubmit(true)
                    } catch (error) {
                      noteFailure(error)
                    }
                    return
                  }
                  const kind = officeKind(appletId)
                  if (team && orgId && kind) {
                    try {
                      const item = await createWorkItem({
                        orgId,
                        kind,
                        idempotencyKey: key,
                        fields: values
                      })
                      notifyWorkInboxChanged()
                      setSubmittedItemId(item.id)
                      setSubmitting(false)
                      rememberSubmit(true)
                    } catch (error) {
                      noteFailure(error)
                    }
                    return
                  }
                  const ok = await submitApplet(appletId, {
                    version: pinned.version,
                    digest: pinned.digest,
                    idempotencyKey: key,
                    values
                  })
                  if (!ok) {
                    setSubmitting(false)
                    setSubmitFailed(true)
                    return
                  }
                  setSubmittedItemId('')
                  setSubmitting(false)
                  rememberSubmit(false)
                })()
              }}>
              <p className="m-0 text-[15px] leading-6 text-[#8a8a8a]">{t(`workbench.applet.${appletId}.lead`)}</p>
              <FieldGroup className="mt-4 gap-4">
                <OfficeAppletFields
                  appletId={appletId}
                  drafts={drafts}
                  onChange={setDrafts}
                  travelLinks={travelLinks}
                  members={approvers}
                />
              </FieldGroup>
              {approverGate === 'ready' && !processKey ? (
                <p data-ui="workbench.applet.approvers" className="m-0 mt-4 text-[13px] leading-5 text-[#3f3f3f]">
                  {t('workbench.applet.approvers')}
                  {approvers.map((person) => person.name).join('、')}
                </p>
              ) : null}
              <ProcessRouteLine
                processKey={processKey}
                approverGate={approverGate}
                snapshot={fieldSnapshot}
                route={route}
              />
              {approverGate === 'empty' ? (
                <p data-ui="workbench.applet.no-approver" className="m-0 mt-4 text-[13px] leading-5 text-[#b42318]">
                  {t('workbench.applet.no_approver')}
                </p>
              ) : null}
              <div className="mt-4 flex items-center justify-end gap-3">
                {submitFailed ? (
                  <p className="m-0 text-[13px] text-[#b42318]">{submitError || t('workbench.applet.submit_failed')}</p>
                ) : null}
                {submitted ? (
                  <p className="m-0 text-[13px] leading-9 text-[#8a8a8a]">
                    {routed ? (
                      <>
                        <span>{t('workbench.applet.submitted.team')}</span>
                        {submittedItemId ? (
                          <span data-ui="workbench.applet.submitted.item" className="ml-2">
                            {submittedItemId}
                          </span>
                        ) : null}
                      </>
                    ) : (
                      t('workbench.applet.submitted')
                    )}
                  </p>
                ) : (
                  <Button type="submit" variant="emphasis" size="md" disabled={submitting || !ready}>
                    {t('workbench.applet.submit')}
                  </Button>
                )}
              </div>
            </form>
          ) : phase === 'form' && pinned ? (
            <div data-ui="workbench.applet.gated" className="h-full" />
          ) : (
            <div
              data-ui={phase === 'signed_out' ? 'workbench.applet.signed-out' : 'workbench.applet.failed'}
              className="flex h-full flex-col items-center justify-center gap-4">
              <span className="size-14">
                <Icon />
              </span>
              <p className="m-0 text-[13px] text-[#8a8a8a]">
                {t(phase === 'signed_out' ? 'workbench.applet.signed_out' : 'workbench.applet.failed')}
              </p>
              <Button type="button" variant="outline" size="sm" onClick={reload}>
                {t('common.retry')}
              </Button>
            </div>
          )}
        </div>
        {phase === 'form' && grant === 'ask' ? (
          <AppletPermissionDialog
            appId={appletId}
            name={title}
            permissions={required}
            onAllow={() => {
              allowPermissionGrant(appletId, required)
              setGrant('allowed')
            }}
            onDeny={() => {
              setProfile(null)
              void runAppletWindowAction('close')
            }}
          />
        ) : null}
      </div>
    </section>
  )
}

function ProcessRouteLine({
  processKey,
  approverGate,
  snapshot,
  route
}: {
  processKey: string | null
  approverGate: 'unknown' | 'personal' | 'ready' | 'empty'
  snapshot: string
  route: ProcessRoute
}) {
  const { t } = useTranslation()
  if (!processKey || approverGate !== 'ready' || snapshot === '') return null
  const current = route.kind !== 'idle' && route.snapshot === snapshot ? route : { kind: 'loading' as const, snapshot }
  if (current.kind === 'refused') {
    return (
      <p data-ui="workbench.applet.route" role="alert" className="m-0 mt-4 text-[13px] leading-5 text-[#b42318]">
        {current.message || t('workbench.applet.submit_failed')}
      </p>
    )
  }
  if (current.kind === 'ok' && processKey === 'clock' && current.nodes.length === 0) {
    return (
      <p data-ui="workbench.applet.route" className="m-0 mt-4 text-[13px] leading-5 text-[#3f3f3f]">
        {t('workbench.applet.clock.recorded')}
      </p>
    )
  }
  if (current.kind === 'ok') {
    return (
      <ol
        data-ui="workbench.applet.route"
        aria-label={t('workbench.applet.route.label')}
        className="m-0 mt-4 flex list-none flex-wrap items-center gap-1.5 p-0">
        {current.nodes.map((node, index) => (
          <li key={`${node.key}:${index}`} className="flex items-center gap-1.5">
            {index > 0 ? (
              <span aria-hidden className="text-[13px] text-[#8a8a8a]">
                →
              </span>
            ) : null}
            <span className="rounded-[8px] bg-[#f2f3f5] px-2.5 py-1 text-[13px] leading-5 text-[#171717]">
              {node.name}
            </span>
          </li>
        ))}
      </ol>
    )
  }
  return (
    <p data-ui="workbench.applet.route" className="m-0 mt-4 text-[13px] leading-5 text-[#8a8a8a]">
      {t('workbench.applet.route.checking')}
    </p>
  )
}

function LeavePortrait({ name, avatar }: { name: string; avatar: string }) {
  if (avatar.startsWith('data:image/') && avatar.includes(';base64,')) {
    return (
      <img
        data-ui="workbench.applet.leave.avatar"
        src={avatar}
        alt={name}
        className="size-10 shrink-0 rounded-full bg-[#f2f3f5] object-cover"
      />
    )
  }
  const letter = Array.from(name.trim())[0] || '?'
  return (
    <span
      data-ui="workbench.applet.leave.avatar"
      role="img"
      aria-label={name}
      className="grid size-10 shrink-0 place-items-center rounded-full bg-[#f2f3f5] text-[15px] font-medium text-[#171717]">
      {letter}
    </span>
  )
}
