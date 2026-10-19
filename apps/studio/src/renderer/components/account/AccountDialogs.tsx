import { Button, Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { Camera, ImageUp, Type } from 'lucide-react'
import { type FormEvent, useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { DefaultAvatar } from '@renderer/components/DefaultAvatar'
import { isSessionDenied, MEMBER_NAME_MAX, type MemberAccount, updateMemberAccount } from '@renderer/data/memberOrgs'
import { cn } from '@renderer/utils/style'

import { AvatarComposer } from './AvatarComposer'
import { avatarDataUrl, avatarSpecFromName, parseAvatarSpec, type AvatarSpec } from './composedAvatar'
import { imageFileToDataUrl, textLogo } from './entityImage'

const IMAGE_ACCEPT = 'image/png,image/jpeg,image/webp,image/gif'

/** Hidden file input behind a button; yields a square data URL or an error message. */
function useImagePicker(onPicked: (dataUrl: string) => void, onError: (message: string) => void) {
  const inputRef = useRef<HTMLInputElement>(null)
  const { t } = useTranslation()
  const input = (
    <input
      ref={inputRef}
      type="file"
      accept={IMAGE_ACCEPT}
      className="hidden"
      tabIndex={-1}
      aria-hidden
      onChange={(event) => {
        const file = event.target.files?.[0]
        event.target.value = ''
        if (!file) return
        if (!file.type.startsWith('image/')) {
          onError(t('sidebar.center.image_type'))
          return
        }
        imageFileToDataUrl(file).then(onPicked, (error: unknown) =>
          onError(error instanceof Error && error.message ? error.message : t('sidebar.center.image_error'))
        )
      }}
    />
  )
  return { input, open: () => inputRef.current?.click() }
}

export function ProfileDialog({
  open,
  account,
  onOpenChange,
  onSaved,
  onSessionDenied,
  save = updateMemberAccount
}: {
  open: boolean
  account: MemberAccount
  onOpenChange: (open: boolean) => void
  onSaved: (next: MemberAccount) => void
  onSessionDenied: () => void
  save?: typeof updateMemberAccount
}) {
  const { t } = useTranslation()
  const nameId = useId()
  const [name, setName] = useState('')
  const [avatar, setAvatar] = useState('')
  const [spec, setSpec] = useState<AvatarSpec>(() => avatarSpecFromName(''))
  const [composerOpen, setComposerOpen] = useState(false)
  const [nameError, setNameError] = useState<string>()
  const [formError, setFormError] = useState<string>()
  const [busy, setBusy] = useState(false)
  const picker = useImagePicker(
    (dataUrl) => {
      setAvatar(dataUrl)
      setComposerOpen(false)
      setFormError(undefined)
    },
    (message) => setFormError(message)
  )

  useEffect(() => {
    if (!open) return
    const parsed = parseAvatarSpec(account.avatar ?? '')
    setName(account.displayName)
    setAvatar(account.avatar ?? '')
    setSpec(parsed ?? avatarSpecFromName(account.displayName))
    setComposerOpen(parsed !== null)
    setNameError(undefined)
    setFormError(undefined)
    setBusy(false)
  }, [account.avatar, account.displayName, open])

  function chooseSpec(next: AvatarSpec) {
    setSpec(next)
    setAvatar(avatarDataUrl(next))
    setFormError(undefined)
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmed = name.trim()
    if (!trimmed) return setNameError(t('sidebar.center.nickname_required'))
    if ([...trimmed].length > MEMBER_NAME_MAX) {
      return setNameError(t('sidebar.center.nickname_too_long', { max: MEMBER_NAME_MAX }))
    }
    setBusy(true)
    setFormError(undefined)
    try {
      const saved = await save({ displayName: trimmed, avatar })
      onSaved({ ...account, ...saved, phone: saved.phone || account.phone })
      onOpenChange(false)
    } catch (error) {
      setBusy(false)
      if (isSessionDenied(error)) {
        onOpenChange(false)
        onSessionDenied()
        return
      }
      setFormError(t('sidebar.center.profile_error'))
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !busy && onOpenChange(next)}>
      <DialogContent
        size="default"
        className="flex max-h-[min(720px,calc(100vh-48px))] flex-col overflow-hidden"
        aria-describedby={undefined}>
        <DialogTitle>{t('sidebar.center.profile_title')}</DialogTitle>
        <form className="flex min-h-0 flex-1 flex-col gap-4" onSubmit={(event) => void onSubmit(event)} noValidate>
          {formError ? (
            <p className="m-0 text-sm text-destructive" role="alert">
              {formError}
            </p>
          ) : null}
          <div className="flex items-center gap-4">
            <button
              type="button"
              className="group relative size-16 shrink-0 overflow-hidden rounded-[12px] outline-none focus-visible:ring-2 focus-visible:ring-black/15 disabled:opacity-60"
              aria-label={t('sidebar.center.change_avatar')}
              disabled={busy}
              onClick={picker.open}>
              <Portrait name={name || account.displayName} avatar={avatar} className="size-16 rounded-[12px] text-lg" />
              <span className="absolute inset-0 flex items-center justify-center bg-black/40 text-white opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
                <Camera className="size-5" strokeWidth={1.7} aria-hidden />
              </span>
            </button>
            <div className="grid min-w-0 gap-1">
              <div className="flex flex-wrap gap-2">
                <Button type="button" size="sm" variant="outline" disabled={busy} onClick={picker.open}>
                  {t('sidebar.center.change_avatar')}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={busy}
                  aria-expanded={composerOpen}
                  onClick={() => {
                    setComposerOpen(true)
                    chooseSpec(spec)
                  }}>
                  {t('sidebar.center.avatar.generate')}
                </Button>
                {avatar ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      setAvatar('')
                      setComposerOpen(false)
                    }}>
                    {t('sidebar.center.remove_avatar')}
                  </Button>
                ) : null}
              </div>
              <p className="m-0 text-xs text-muted-foreground">{t('sidebar.center.image_hint')}</p>
            </div>
            {picker.input}
          </div>
          {composerOpen ? (
            <div data-ui="account.avatar-options" className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
              <AvatarComposer spec={spec} disabled={busy} onChange={chooseSpec} />
            </div>
          ) : null}
          <label className="grid gap-1.5 text-sm" htmlFor={nameId}>
            {t('sidebar.center.nickname')}
            <Input
              id={nameId}
              name="display-name"
              value={name}
              maxLength={MEMBER_NAME_MAX}
              disabled={busy}
              autoFocus
              placeholder={t('sidebar.center.nickname_placeholder')}
              aria-invalid={nameError ? true : undefined}
              onChange={(event) => {
                setName(event.target.value)
                setNameError(undefined)
              }}
            />
          </label>
          {nameError ? (
            <p className="m-0 -mt-2 text-sm text-destructive" role="alert">
              {nameError}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" size="sm" disabled={busy}>
              {t('common.save')}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}

/** 团队 Logo：上传图片，或按团队名称自动生成文字 Logo。 */
export function TeamLogoField({
  name,
  logo,
  disabled,
  onChange,
  onError
}: {
  name: string
  /** 上传的图片；空串表示使用文字 Logo。 */
  logo: string
  disabled?: boolean
  onChange: (logo: string) => void
  onError: (message: string) => void
}) {
  const { t } = useTranslation()
  const picker = useImagePicker(onChange, onError)
  const preview = logo || textLogo(name)
  return (
    <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
      <span>{t('sidebar.center.logo')}</span>
      <div className="flex items-center gap-3">
        <button
          type="button"
          className="size-14 shrink-0 overflow-hidden rounded-[12px] outline-none focus-visible:ring-2 focus-visible:ring-black/15 disabled:opacity-60"
          aria-label={t('sidebar.center.upload_logo')}
          disabled={disabled}
          onClick={picker.open}>
          {preview ? (
            <img src={preview} alt="" data-ui="account.team-logo" className="size-14 object-cover" />
          ) : (
            <span className="flex size-14 items-center justify-center border border-dashed border-[#d9d9d9] text-[#a3a3a3] dark:border-[#3a3a3a]">
              <ImageUp className="size-5" strokeWidth={1.6} aria-hidden />
            </span>
          )}
        </button>
        <div className="grid min-w-0 gap-1">
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={disabled}
              onClick={picker.open}
              className="inline-flex h-[30px] items-center justify-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] leading-none text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
              <ImageUp className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('sidebar.center.upload_logo')}
            </button>
            {logo ? (
              <button
                type="button"
                disabled={disabled}
                onClick={() => onChange('')}
                className="inline-flex h-[30px] items-center justify-center gap-1.5 rounded-[8px] px-2 text-[13px] leading-none text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                <Type className="size-3.5" strokeWidth={1.7} aria-hidden />
                {t('sidebar.center.use_text_logo')}
              </button>
            ) : null}
          </div>
          <p className="m-0 text-[12px] text-[#8a8a8a]">
            {logo ? t('sidebar.center.image_hint') : t('sidebar.center.logo_hint')}
          </p>
        </div>
        {picker.input}
      </div>
    </div>
  )
}

export function Portrait({ name, avatar, className }: { name: string; avatar?: string; className?: string }) {
  const label = name || ' '
  if (avatar && avatar.startsWith('data:image/')) {
    return <img src={avatar} alt={label} className={cn('shrink-0 bg-[#d6d6d6] object-cover', className)} />
  }
  return <DefaultAvatar label={label} className={className} />
}
