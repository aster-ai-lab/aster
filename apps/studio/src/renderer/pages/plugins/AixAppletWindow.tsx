import { ConfirmDialog } from '@a3s-lab/ui'
import { Package } from 'lucide-react'
import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { publishMemberProfile } from '@renderer/components/account/memberProfile'
import { ToWorkPageLoading } from '@renderer/components/layout/PageEntry'
import { AixWorkbenchRuntime } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { readMemberAccount } from '@renderer/data/memberOrgs'
import { AppletWindowBar } from '@renderer/pages/agents/AppletWindowBar'
import { currentAppletWindowLabel } from '@renderer/pages/agents/workbenchAppletWindowBridge'
import {
  findAixWindow,
  checkpointAixEditor,
  closeRecoveredAixWindow,
  listenAppletHomeSeed,
  readAixCatalogue,
  subscribeAixCatalogue,
  takeAndStageAppletHomeSeed
} from '@renderer/services/plugins'

const logger = loggerService.withContext('AixAppletWindow')
const NO_BUILTINS = [] as const
/** A catalogue that never names the package within this time is a missing mini app. */
const CATALOGUE_PATIENCE_MS = 10_000
const MEMBER_ATTEMPTS = 8

function Missing() {
  const { t } = useTranslation()
  return (
    <div role="status" className="flex flex-1 items-center justify-center p-6 text-sm text-muted-foreground">
      {t('sidebar.resource_missing')}
    </div>
  )
}

function AixAppletPage({ slug, view }: { slug: string; view: string }) {
  const { t } = useTranslation()
  const catalogue = useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
  const [waiting, setWaiting] = useState(true)
  useEffect(() => {
    const timer = globalThis.setTimeout(() => setWaiting(false), CATALOGUE_PATIENCE_MS)
    return () => globalThis.clearTimeout(timer)
  }, [])
  const target = findAixWindow(catalogue, slug, view)
  const loaded = catalogue.revision !== '' || catalogue.packages.length > 0
  const missing = !target && (loaded || !waiting)
  useEffect(() => {
    if (missing) logger.warn('AIX mini app is not in the installed catalogue', { slug, view, loaded })
  }, [missing, slug, view, loaded])
  if (target) return <PluginPage page={target.page} allowBuiltins={false} />
  return missing ? <Missing /> : <ToWorkPageLoading label={t('common.loading')} />
}

/** Native window for an installed AIX editor or container, with that package's sandboxed views only. */
export function AixAppletWindow({ slug, view, title }: { slug: string; view: string; title: string }) {
  const { t } = useTranslation()
  const [member, setMember] = useState<'pending' | 'ready' | 'denied'>('pending')
  useEffect(() => {
    const pull = () => {
      void takeAndStageAppletHomeSeed(slug, view)
    }
    pull()
    return listenAppletHomeSeed(pull)
  }, [slug, view])
  const [reloadKey, setReloadKey] = useState(0)
  const [leaving, setLeaving] = useState<'close' | 'reload' | null>(null)
  const [leaveError, setLeaveError] = useState(false)
  const checking = useRef(false)
  const requestClose = useRef<() => void>(() => {})
  const [memberKey, setMemberKey] = useState(0)
  useEffect(() => {
    let cancelled = false
    let timer: ReturnType<typeof globalThis.setTimeout> | undefined
    setMember('pending')
    // The installed catalogue is keyed by member id; the session fallback has none while the OS is unreachable.
    const attempt = (count: number) => {
      void readMemberAccount()
        .then((account) => {
          if (cancelled) return
          if (account.id) {
            publishMemberProfile(account)
            setMember('ready')
          } else if (count + 1 < MEMBER_ATTEMPTS) {
            timer = globalThis.setTimeout(() => attempt(count + 1), Math.min(1000 * 2 ** count, 8000))
          } else {
            logger.warn('AIX mini app member account has no id', { attempts: MEMBER_ATTEMPTS })
            setMember('denied')
          }
        })
        .catch((error: unknown) => {
          if (cancelled) return
          logger.warn(
            'AIX mini app member account unavailable',
            error instanceof Error ? error : new Error(String(error))
          )
          setMember('denied')
        })
    }
    attempt(0)
    return () => {
      cancelled = true
      globalThis.clearTimeout(timer)
    }
  }, [memberKey])
  useEffect(() => {
    if (member !== 'denied') return undefined
    const retry = () => setMemberKey((current) => current + 1)
    globalThis.window.addEventListener('focus', retry)
    return () => globalThis.window.removeEventListener('focus', retry)
  }, [member])
  const reload = () => {
    if (member === 'ready') setReloadKey((current) => current + 1)
    else setMemberKey((current) => current + 1)
  }
  const performLeave = async (action: 'close' | 'reload') => {
    if (action === 'close') await closeRecoveredAixWindow()
    else reload()
  }
  const requestLeave = async (action: 'close' | 'reload') => {
    if (checking.current || leaving) return
    checking.current = true
    setLeaveError(false)
    try {
      if (await checkpointAixEditor(slug, view)) setLeaving(action)
      else await performLeave(action)
    } catch (error) {
      logger.warn('Editor checkpoint refused navigation', error instanceof Error ? error : new Error(String(error)))
      setLeaveError(true)
    } finally {
      checking.current = false
    }
  }
  requestClose.current = () => {
    void requestLeave('close')
  }
  useEffect(() => {
    const close = () => requestClose.current()
    globalThis.window.addEventListener('a3s:aix-editor-close', close)
    return () => globalThis.window.removeEventListener('a3s:aix-editor-close', close)
  }, [])
  return (
    <div className="fixed inset-0 flex w-full flex-col overflow-hidden bg-white">
      <AppletWindowBar
        title={title || slug}
        icon={<Package className="size-4 text-[#171717]" strokeWidth={1.7} aria-hidden />}
        onHome={() => {
          void requestLeave('reload')
        }}
        onRefresh={() => {
          void requestLeave('reload')
        }}
        onClose={() => {
          void requestLeave('close')
        }}
      />
      {leaveError ? (
        <p role="alert" className="m-0 px-4 py-2 text-sm text-[#b42318]">
          {t('plugins.recovery.failed')}
        </p>
      ) : null}
      <div className="flex min-h-0 w-full flex-1 flex-col">
        {member === 'ready' ? (
          <AixWorkbenchRuntime
            key={reloadKey}
            windowId={currentAppletWindowLabel() ?? 'aix-applet'}
            builtins={NO_BUILTINS}
            packageSlug={slug}>
            <AixAppletPage slug={slug} view={view} />
          </AixWorkbenchRuntime>
        ) : member === 'denied' ? (
          <Missing />
        ) : (
          <ToWorkPageLoading label={t('common.loading')} />
        )}
      </div>
      <ConfirmDialog
        open={leaving !== null}
        onOpenChange={(open) => {
          if (!open) setLeaving(null)
        }}
        title={t('plugins.recovery.title')}
        description={t('plugins.recovery.description')}
        confirmText={t('plugins.recovery.continue')}
        cancelText={t('common.cancel')}
        failureText={t('plugins.recovery.failed')}
        operationKey={`${slug}/${view}/${leaving}`}
        onConfirm={async () => {
          if (!leaving) return false
          await checkpointAixEditor(slug, view)
          await performLeave(leaving)
        }}
      />
    </div>
  )
}
