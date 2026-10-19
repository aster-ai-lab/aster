import { Button, Skeleton } from '@a3s-lab/ui'
import { A3s } from '@a3s-lab/ui/icons/providers'
import { RefreshCw } from 'lucide-react'
import type { FC } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Trans, useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { osGatewayOrigin } from '@renderer/data/osGateway'
import { useProvider } from '@renderer/hooks/useProvider'
import { ipcApi } from '@renderer/ipc'
import { oauthCardClasses } from '@renderer/pages/settings/ProviderSettings/primitives/ProviderSettingsPrimitives'
import { oauthWithA3s } from '@renderer/services/oauth'
import { popup } from '@renderer/services/popup'
import { toast } from '@renderer/services/toast'
import { openExternalWebsite } from '@renderer/services/website'
import { studioModelServiceLogo, studioModelServiceName } from '@renderer/tenantBrand'
import { cn } from '@renderer/utils/style'
import { IpcError } from '@shared/ipc/errors/IpcError'
import { oauthErrorCodes } from '@shared/ipc/errors/oauth'
import type { A3sBalance } from '@shared/ipc/schemas/a3s'
import { hasApiKeys } from '@shared/utils/provider'
import { SystemProviderIds } from '@shared/utils/systemProviderId'

const logger = loggerService.withContext('A3sOauth')

function gatewayOrigin(): string {
  return osGatewayOrigin()
}

interface A3sOauthProps {
  providerId: string
}

function ModelServiceMark({ size }: { size: number }) {
  const logo = studioModelServiceLogo()
  if (!logo) return <A3s.Avatar shape="circle" size={size} />
  return (
    <img
      src={logo}
      alt=""
      width={size}
      height={size}
      data-ui="settings.provider.model-service.logo"
      className="shrink-0 rounded-full object-cover"
      draggable={false}
    />
  )
}

function formatCurrency(value: number | null | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) {
    return '-'
  }

  return `$${value.toFixed(2)}`
}

const A3sOauth: FC<A3sOauthProps> = ({ providerId }) => {
  const { provider, updateProvider, addApiKey, deleteApiKey } = useProvider(providerId)
  const { t } = useTranslation()
  const serviceName = studioModelServiceName()
  const serviceText = (key: string) => {
    const text = t(key)
    if (!serviceName) return text
    return text.replaceAll('A3S_MODEL', serviceName).replaceAll('A3S', serviceName)
  }

  const [isLoggingIn, setIsLoggingIn] = useState(false)
  const [isCancellingLogin, setIsCancellingLogin] = useState(false)
  const [isLoggingOut, setIsLoggingOut] = useState(false)
  const [isLoadingData, setIsLoadingData] = useState(false)
  const [balanceInfo, setBalanceInfo] = useState<A3sBalance | null>(null)
  const [oauthTokenOverride, setOauthTokenOverride] = useState<boolean | null>(null)
  // `oauth.has_token` returns only a boolean — the access/refresh tokens stay in
  // the main process and never reach the renderer (null = status not loaded yet).
  const [remoteHasOAuthToken, setRemoteHasOAuthToken] = useState<boolean | null>(null)
  const topupInProgressRef = useRef(false)
  const signInRequestIdRef = useRef<string | null>(null)

  const refreshHasToken = useCallback(async () => {
    try {
      setRemoteHasOAuthToken(await ipcApi.request('oauth.has_token', { providerId: SystemProviderIds.a3s }))
    } catch (error) {
      logger.warn('Failed to check A3S OAuth token status:', error as Error)
      setRemoteHasOAuthToken(false)
    }
  }, [])

  useEffect(() => {
    void refreshHasToken()
  }, [refreshHasToken])

  const hasKeys = provider ? hasApiKeys(provider) : false
  const hasOAuthToken = oauthTokenOverride ?? remoteHasOAuthToken ?? false
  const isOAuthLoggedIn = hasKeys && hasOAuthToken

  const fetchData = useCallback(async () => {
    setIsLoadingData(true)
    try {
      const balance = await ipcApi.request('a3s.get_balance', { apiHost: gatewayOrigin() })
      setBalanceInfo(balance)
    } catch (error) {
      logger.warn('Failed to fetch balance:', error as Error)
      setBalanceInfo(null)
    } finally {
      setIsLoadingData(false)
    }
  }, [])

  useEffect(() => {
    if (isOAuthLoggedIn) {
      void fetchData()
    } else {
      setBalanceInfo(null)
    }
  }, [fetchData, isOAuthLoggedIn])

  useEffect(() => {
    if (oauthTokenOverride !== null && remoteHasOAuthToken !== null && remoteHasOAuthToken === oauthTokenOverride) {
      setOauthTokenOverride(null)
    }
  }, [oauthTokenOverride, remoteHasOAuthToken])

  // Top-up happens in the system browser,
  // so the balance refresh must wait for the user to come back.
  useEffect(() => {
    const handleWindowFocus = () => {
      if (topupInProgressRef.current) {
        topupInProgressRef.current = false
        void fetchData()
      }
    }
    window.addEventListener('focus', handleWindowFocus)
    return () => window.removeEventListener('focus', handleWindowFocus)
  }, [fetchData])

  const handleOAuthLogin = useCallback(async () => {
    if (signInRequestIdRef.current) return

    const requestId = crypto.randomUUID()
    signInRequestIdRef.current = requestId
    setIsLoggingIn(true)
    try {
      await oauthWithA3s(
        async (apiKeys: string) => {
          const keys = apiKeys
            .split(',')
            .map((key) => key.trim())
            .filter(Boolean)

          await Promise.all(keys.map((key) => addApiKey(key, 'OAuth')))
          await updateProvider({ isEnabled: true })
          setOauthTokenOverride(true)
          void refreshHasToken()
          await fetchData()
          toast.success(t('auth.get_key_success'))
        },
        {
          oauthServer: gatewayOrigin(),
          requestId
        }
      )
    } catch (error) {
      if (error instanceof IpcError && error.code === oauthErrorCodes.SIGN_IN_CANCELLED) return
      logger.error('OAuth error:', error as Error)
      toast.error(t('settings.provider.oauth.error'))
    } finally {
      if (signInRequestIdRef.current === requestId) {
        signInRequestIdRef.current = null
        setIsLoggingIn(false)
      }
    }
  }, [addApiKey, fetchData, refreshHasToken, t, updateProvider])

  const handleCancelLogin = useCallback(async () => {
    const requestId = signInRequestIdRef.current
    if (!requestId) return

    setIsCancellingLogin(true)
    try {
      await ipcApi.request('oauth.cancel_sign_in', { providerId: SystemProviderIds.a3s, requestId })
    } catch (error) {
      logger.error('Failed to cancel A3S OAuth login:', error as Error)
      toast.error(t('settings.provider.oauth.error'))
    } finally {
      setIsCancellingLogin(false)
    }
  }, [t])

  const handleLogout = useCallback(async () => {
    const confirmed = await popup.confirm({
      title: t('settings.provider.oauth.logout'),
      content: t('settings.provider.oauth.logout_confirm'),
      centered: true
    })
    if (!confirmed) return

    setIsLoggingOut(true)

    try {
      await ipcApi.request('a3s.logout', { apiHost: gatewayOrigin() })
      setOauthTokenOverride(false)
      setBalanceInfo(null)

      void refreshHasToken()

      const oauthKeys = provider?.apiKeys.filter((key) => key.label === 'OAuth') ?? []
      const deleteResults = await Promise.allSettled(oauthKeys.map((key) => deleteApiKey(key.id)))
      const rejectedDeletes = deleteResults.filter((result) => result.status === 'rejected')
      if (rejectedDeletes.length > 0) {
        logger.warn(`Failed to delete ${rejectedDeletes.length} A3S OAuth key(s) after logout`)
        toast.warning(t('settings.provider.oauth.logout_warning'))
        return
      }

      toast.success(t('settings.provider.oauth.logout_success'))
    } catch (error) {
      logger.error('Logout error:', error as Error)
      toast.warning(t('settings.provider.oauth.logout_warning'))
    } finally {
      setIsLoggingOut(false)
    }
  }, [deleteApiKey, provider?.apiKeys, refreshHasToken, t])

  const handleTopup = useCallback(() => {
    topupInProgressRef.current = true
    void openExternalWebsite(`${gatewayOrigin()}/console/topup`)
  }, [])

  if (!provider) {
    return null
  }

  if (remoteHasOAuthToken === null && hasKeys) {
    return (
      <div className={oauthCardClasses.container}>
        <div className={oauthCardClasses.shell}>
          <Skeleton className="h-5 w-55" />
          <Skeleton className="mt-2 h-4 w-full" />
          <Skeleton className="mt-2 h-4 w-[82%]" />
        </div>
      </div>
    )
  }

  if (!isOAuthLoggedIn) {
    return (
      <div className={oauthCardClasses.container}>
        <div className={oauthCardClasses.shell}>
          <div className={oauthCardClasses.loggedInRow}>
            <div className={oauthCardClasses.profileMeta}>
              <ModelServiceMark size={40} />
              <div className={oauthCardClasses.nameBlock}>
                <div className={oauthCardClasses.loggedInName}>{t('settings.provider.oauth.a3s.not_logged_in')}</div>
                <div className={cn(oauthCardClasses.loggedInEmail, 'text-muted-foreground')}>
                  {t('settings.provider.oauth.a3s.tagline')}
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="emphasis" onClick={handleOAuthLogin} disabled={isLoggingIn}>
                {isLoggingIn ? <RefreshCw className="size-4 animate-spin" aria-hidden /> : null}
                {serviceText('settings.provider.oauth.a3s.login_button')}
              </Button>
              {isLoggingIn ? (
                <Button variant="outline" onClick={handleCancelLogin} disabled={isCancellingLogin}>
                  {t('common.cancel')}
                </Button>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    )
  }

  const profileName =
    balanceInfo?.profile?.displayName ||
    balanceInfo?.profile?.username ||
    balanceInfo?.profile?.email ||
    serviceName ||
    provider.name
  const profileEmail = balanceInfo?.profile?.email || t('settings.provider.oauth.a3s.logged_in')
  const profileGroup =
    balanceInfo?.profile?.group && balanceInfo.profile.group !== 'default' ? balanceInfo.profile.group : null

  return (
    <div className={oauthCardClasses.container}>
      <div className={cn(oauthCardClasses.shellLoggedIn, 'text-muted-foreground')}>
        <div className={oauthCardClasses.loggedInRow}>
          <div className={oauthCardClasses.profileMeta}>
            <ModelServiceMark size={40} />
            <div className={oauthCardClasses.nameBlock}>
              <div className={oauthCardClasses.nameRow}>
                <div className={cn(oauthCardClasses.loggedInName, 'text-foreground')}>{profileName}</div>
                {profileGroup ? <span className={oauthCardClasses.badge}>{profileGroup}</span> : null}
              </div>
              <div className={cn(oauthCardClasses.loggedInEmail, 'text-muted-foreground')}>{profileEmail}</div>
            </div>
          </div>
          <div className={cn(oauthCardClasses.loggedInActions, 'gap-1.5')}>
            <div className={cn(oauthCardClasses.inlineBalanceBlock, 'mr-1 flex items-baseline gap-1.5 text-left')}>
              <p className={cn(oauthCardClasses.inlineBalanceLabel, 'text-muted-foreground')}>
                {t('settings.provider.oauth.balance')}
              </p>
              <div className={cn(oauthCardClasses.inlineBalanceValue, 'text-foreground')}>
                {isLoadingData && !balanceInfo ? (
                  <Skeleton className={`${oauthCardClasses.balanceValueSkeleton} h-5`} />
                ) : (
                  formatCurrency(balanceInfo?.balance)
                )}
              </div>
            </div>
            <Button
              className={cn(oauthCardClasses.topupPrimaryButton, 'h-7 px-2.5 py-0')}
              onClick={handleTopup}
              size="sm"
              variant="default">
              {t('settings.provider.oauth.topup')}
            </Button>
            <Button
              className={cn(oauthCardClasses.logoutCompact, 'h-7 px-2 py-0 text-muted-foreground')}
              disabled={isLoggingOut}
              onClick={handleLogout}
              variant="ghost">
              {t('settings.provider.oauth.logout')}
            </Button>
          </div>
        </div>
        {serviceName ? null : (
          <p className={cn(oauthCardClasses.serviceAttribution, 'text-muted-foreground')}>
            <Trans
              i18nKey="settings.provider.oauth.a3s.service_attribution"
              components={{
                link: (
                  <a
                    key="a3s-service-link"
                    className={cn(oauthCardClasses.serviceLink, 'text-muted-foreground')}
                    href={gatewayOrigin()}
                    rel="noreferrer"
                    target="_blank"
                  />
                )
              }}
            />
          </p>
        )}
      </div>
    </div>
  )
}

export default A3sOauth
