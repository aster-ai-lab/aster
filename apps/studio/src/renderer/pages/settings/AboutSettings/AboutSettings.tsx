import { useLocation, useNavigate, useSearch } from '@tanstack/react-router'
import { debounce } from 'es-toolkit/compat'
import { ChevronRight, Info } from 'lucide-react'
import type { FC } from 'react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { DoctorPopup } from '@renderer/components/doctor'
import { SettingGroup, SettingsContentColumn } from '@renderer/components/SettingsPrimitives'
import { StudioProductMark } from '@renderer/components/StudioProductMark'
import { useAppUpdateState } from '@renderer/hooks/useAppUpdateState'
import { useTheme } from '@renderer/hooks/useTheme'
import { ipcApi } from '@renderer/ipc'
import { toast } from '@renderer/services/toast'
import { studioProductName } from '@renderer/tenantBrand'
import { clearManualAppUpdateCheck, markManualAppUpdateCheck } from '@renderer/windows/main/hooks/useAppUpdateHandler'
import { DOCTOR_OPEN_QUERY_PARAM, type DoctorPanel } from '@shared/utils/doctor'
import { localizeReleaseNotes } from '@shared/utils/releaseNotes'

import { openNoticeWindow } from './openNoticeWindow'
import { publishedNoticeUrl, type NoticeSlug } from './publicationLink'

function bundledAppVersion(): string {
  return typeof __APP_RELEASE_VERSION__ === 'string' ? __APP_RELEASE_VERSION__ : ''
}

/** Studio marketing version, matching the app bundle short version (`v2.1.3`). */
export function studioVersionLabel(version: string): string {
  const text = version.trim().replace(/^v/i, '')
  return text
}

/** First changelog bullet, skipping notes that still name another product. */
export function aboutChangelogBullet(source: string, language: string): string {
  const text = localizeReleaseNotes(source, language).trim()
  if (!text || /cherry studio/i.test(text)) return ''
  const line = text
    .split('\n')
    .map((item) => item.trim())
    .find((item) => item.startsWith('- ') || item.startsWith('* '))
  if (!line) return ''
  return line
    .replace(/^[-*]\s*/, '')
    .replace(/^\[[^\]]+\]\s*/, '')
    .trim()
}

type ReleaseVersion = {
  major: number
  minor: number
  patch: number
  pre: string[] | null
}

/** Whether the published release is newer than the installed build. `null` means a version could not be read. */
export function releaseIsNewer(remote: string, local: string): boolean | null {
  const published = parseReleaseVersion(remote)
  const installed = parseReleaseVersion(local)
  if (!published || !installed) return null
  return compareRelease(published, installed) > 0
}

export type AboutReleaseFeed =
  | { kind: 'loading' }
  | { kind: 'current'; notes: string }
  | { kind: 'newer'; version: string; notes: string }
  | { kind: 'unpublished' }
  | { kind: 'failed' }

/** Map one published desktop release onto the about-page status. */
export function aboutFeedFromRelease(
  installed: string,
  release: { version?: string; releaseNotes?: string } | null
): Exclude<AboutReleaseFeed, { kind: 'loading' }> {
  const version = release?.version?.trim() ?? ''
  if (!version) return { kind: 'unpublished' }
  const newer = releaseIsNewer(version, installed)
  if (newer === null) return { kind: 'failed' }
  const notes = release?.releaseNotes ?? ''
  return newer ? { kind: 'newer', version, notes } : { kind: 'current', notes }
}

function parseReleaseVersion(raw: string): ReleaseVersion | null {
  const text = raw.trim()
  if (!text || text.length > 40) return null
  const splitAt = text.search(/[-+]/)
  const core = splitAt === -1 ? text : text.slice(0, splitAt)
  const pre = splitAt === -1 ? null : text.slice(splitAt + 1)
  if (pre !== null && pre.length === 0) return null
  const parts = core.split('.')
  if (parts.length !== 3) return null
  const numbers = parts.map((part) => (/^\d+$/.test(part) ? Number(part) : Number.NaN))
  if (numbers.some((part) => !Number.isSafeInteger(part))) return null
  return { major: numbers[0], minor: numbers[1], patch: numbers[2], pre: pre?.split('.') ?? null }
}

function compareRelease(left: ReleaseVersion, right: ReleaseVersion): number {
  const core = left.major - right.major || left.minor - right.minor || left.patch - right.patch
  if (core !== 0) return core
  const leftPre = left.pre
  const rightPre = right.pre
  if (!leftPre && !rightPre) return 0
  if (leftPre && !rightPre) return -1
  if (!leftPre || !rightPre) return 1
  const count = Math.max(leftPre.length, rightPre.length)
  for (let index = 0; index < count; index += 1) {
    const leftId = leftPre[index]
    const rightId = rightPre[index]
    if (leftId === undefined) return -1
    if (rightId === undefined) return 1
    const order = compareReleaseId(leftId, rightId)
    if (order !== 0) return order
  }
  return 0
}

function compareReleaseId(leftId: string, rightId: string): number {
  const leftNum = /^\d+$/.test(leftId) ? Number(leftId) : null
  const rightNum = /^\d+$/.test(rightId) ? Number(rightId) : null
  if (leftNum !== null && rightNum !== null) return leftNum - rightNum
  if (leftNum !== null) return -1
  if (rightNum !== null) return 1
  if (leftId < rightId) return -1
  if (leftId > rightId) return 1
  return 0
}

const quietButtonClassName =
  'inline-flex h-[30px] shrink-0 items-center justify-center rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] leading-none text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40'

const AboutSettings: FC = () => {
  const { t } = useTranslation()
  const { theme } = useTheme()
  const productName = studioProductName()
  const location = useLocation()
  const navigate = useNavigate()
  const search = useSearch({ strict: false }) as Partial<Record<typeof DOCTOR_OPEN_QUERY_PARAM, DoctorPanel>>
  const consumedDoctorPanelRef = useRef<DoctorPanel | undefined>(undefined)
  const { appUpdateState, updateAppUpdateState } = useAppUpdateState()
  const [version, setVersion] = useState(bundledAppVersion)
  const [isPortable, setIsPortable] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [feed, setFeed] = useState<AboutReleaseFeed>({ kind: 'loading' })

  useEffect(() => {
    const initialPanel = search[DOCTOR_OPEN_QUERY_PARAM]
    if (!initialPanel) {
      consumedDoctorPanelRef.current = undefined
      return
    }
    if (consumedDoctorPanelRef.current === initialPanel) return
    consumedDoctorPanelRef.current = initialPanel

    void navigate({
      to: location.pathname,
      search: (previous: Record<string, unknown>) => {
        const remaining = { ...previous }
        delete remaining[DOCTOR_OPEN_QUERY_PARAM]
        return remaining
      },
      replace: true
    })
    void DoctorPopup.show({ initialPanel })
  }, [location.pathname, navigate, search])

  useEffect(() => {
    let cancelled = false
    void ipcApi
      .request('app.get_info')
      .then((info) => {
        if (cancelled || !info || typeof info !== 'object') return
        if (typeof info.version === 'string' && info.version) setVersion(info.version)
        if (info.isPortable === true) setIsPortable(true)
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!version) return
    let cancelled = false
    setFeed({ kind: 'loading' })
    void ipcApi
      .request('app.updater.release_notes.get')
      .then((rows) => {
        if (cancelled) return
        const row = Array.isArray(rows) ? rows[0] : null
        setFeed(aboutFeedFromRelease(version, row ?? null))
      })
      .catch(() => {
        if (!cancelled) setFeed({ kind: 'failed' })
      })
    return () => {
      cancelled = true
    }
  }, [version])

  const onCheckUpdate = debounce(
    async () => {
      if (appUpdateState.checking || appUpdateState.downloading || installing) return
      markManualAppUpdateCheck()
      updateAppUpdateState({ checking: true, manualCheck: true })
      try {
        await ipcApi.request('app.updater.check_for_update')
      } catch {
        clearManualAppUpdateCheck()
        updateAppUpdateState({ manualCheck: false })
        toast.error(t('settings.about.updateError'))
      }
      updateAppUpdateState({ checking: false })
    },
    2000,
    { leading: true, trailing: false }
  )

  const onRestart = () => {
    setInstalling(true)
    // A resolved call means the process is still here, so the install did not finish.
    void ipcApi
      .request('app.updater.quit_and_install')
      .then(() => {
        setInstalling(false)
      })
      .catch(() => {
        setInstalling(false)
        toast.error(t('settings.about.updateError'))
      })
  }

  const updateReady = installing || (appUpdateState.downloaded && !appUpdateState.downloading)
  const showProgress = appUpdateState.downloading || installing
  const progress = Math.max(0, Math.min(100, Math.round(appUpdateState.downloadProgress)))
  const offerUpdate = !updateReady && (appUpdateState.available || feed.kind === 'newer')
  const confirming = !version || feed.kind === 'loading'
  const status = showProgress
    ? t(installing ? 'settings.about.installing' : 'settings.about.downloading')
    : appUpdateState.checking || confirming
      ? t('settings.about.page.checking')
      : updateReady
        ? t('settings.about.page.update_ready', { name: productName })
        : offerUpdate
          ? t('settings.about.updateAvailable', {
              version: appUpdateState.info?.version || (feed.kind === 'newer' ? feed.version : '')
            })
          : feed.kind === 'failed'
            ? t('settings.about.updateError')
            : feed.kind === 'unpublished'
              ? t('settings.about.updateNotPublished')
              : t('settings.about.page.up_to_date')

  const openNotice = async (slug: NoticeSlug, title: string) => {
    const url = await publishedNoticeUrl(slug)
    if (!url) {
      toast.info(t('settings.about.page.link_unavailable'))
      return
    }
    const opened = await openNoticeWindow({ slug, title, url })
    if (!opened) toast.error(t('settings.about.page.open_failed'))
  }

  const links = [
    {
      id: 'terms',
      label: t('settings.about.page.terms'),
      onOpen: () => openNotice('terms', t('settings.about.page.terms'))
    },
    {
      id: 'privacy',
      label: t('settings.about.page.privacy'),
      onOpen: () => openNotice('privacy', t('settings.about.page.privacy'))
    },
    {
      id: 'copyright',
      label: t('settings.about.page.copyright'),
      onOpen: () => openNotice('copyright', t('settings.about.page.copyright'))
    },
    {
      id: 'support',
      label: t('settings.about.page.support'),
      onOpen: () => openNotice('support', t('settings.about.page.support'))
    }
  ] as const

  return (
    <SettingsContentColumn theme={theme} className="pt-16">
      <h1 aria-label={productName} className="m-0 mb-6 flex items-center">
        <StudioProductMark
          className={
            productName === 'ToWork' || productName === 'AsterDream'
              ? 'h-14'
              : 'text-[28px] leading-none font-semibold tracking-tight text-[#1a1a1a]'
          }
        />
      </h1>

      <SettingGroup theme={theme} data-ui="settings.about" className="mt-0 overflow-hidden p-0">
        <div
          id="setting-about-update"
          aria-busy={appUpdateState.checking || confirming}
          className="scroll-mt-6 px-4 pt-4 pb-4">
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 items-start gap-2.5">
              <Info className="mt-0.5 size-4 shrink-0 text-[#1a1a1a]" strokeWidth={1.7} aria-hidden />
              <div className="min-w-0">
                <p className="text-[14px] leading-5 font-medium text-[#1a1a1a]">{status}</p>
                <p className="mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                  {t('settings.about.page.version', { version: studioVersionLabel(version) })}
                </p>
              </div>
            </div>
            {!isPortable &&
              (updateReady ? (
                <button type="button" onClick={onRestart} disabled={installing} className={quietButtonClassName}>
                  {t('settings.about.page.restart')}
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => void onCheckUpdate()}
                  disabled={appUpdateState.checking || appUpdateState.downloading || installing || confirming}
                  className={quietButtonClassName}>
                  {t(offerUpdate ? 'settings.about.checkUpdate.available' : 'settings.about.checkUpdate.label')}
                </button>
              ))}
          </div>
          {showProgress ? (
            <div className="mt-3 flex items-center gap-3 pl-[26px]">
              <div
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progress}
                aria-label={status}
                className="h-1 min-w-0 flex-1 overflow-hidden rounded-full bg-[#efefef]">
                <div className="h-full rounded-full bg-[#171717]" style={{ width: `${progress}%` }} />
              </div>
              <span className="w-9 shrink-0 text-right text-[12px] leading-4 text-[#8a8a8a] tabular-nums">
                {progress}%
              </span>
            </div>
          ) : null}
        </div>

        <ul className="border-t border-[#efefef]">
          {links.map((link) => (
            <li key={link.id} className="border-b border-[#efefef] last:border-b-0">
              <button
                id={`setting-about-${link.id}`}
                type="button"
                onClick={link.onOpen}
                className="flex h-9 w-full items-center justify-between px-4 text-left text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                <span>{link.label}</span>
                <ChevronRight className="size-3.5 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
              </button>
            </li>
          ))}
        </ul>
      </SettingGroup>
    </SettingsContentColumn>
  )
}

export default AboutSettings
