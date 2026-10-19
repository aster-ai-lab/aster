import { Button, Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import {
  TOBOSS_PROFILE_KEY,
  TOBOT_COLORS,
  TOBOT_EXPRESSIONS,
  TOBOT_EYES,
  TOBOT_PROFILE_KEY,
  TOBOT_SHAPES,
  tobotDisplayName,
  useBotProfile,
  writeBotProfile,
  type ToBotProfile
} from '@renderer/im/tobotProfile'
import { cn } from '@renderer/utils/style'

import { ToBossMark } from './ToBossMark'
import { TOBOT_COLOR_HEX } from './tobotAvatar'
import { ToBotMark } from './ToBotMark'

export function ToBotProfileDialog({
  open,
  onOpenChange,
  storageKey = TOBOT_PROFILE_KEY,
  fallbackKey = 'messages.file_transfer'
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  storageKey?: string
  fallbackKey?: string
}) {
  const { t } = useTranslation()
  const stored = useBotProfile(storageKey)
  const [draft, setDraft] = useState<ToBotProfile>(stored)

  useEffect(() => {
    if (open) setDraft(stored)
  }, [open, stored])

  const label = tobotDisplayName(draft, t(fallbackKey))
  const fixedFace = storageKey === TOBOSS_PROFILE_KEY
  const set = (patch: Partial<ToBotProfile>) => setDraft((current) => ({ ...current, ...patch }))
  const save = () => {
    writeBotProfile(storageKey, draft)
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        size="lg"
        closeLabel={t('common.close')}
        aria-describedby={undefined}
        data-ui="messages.tobot-profile"
        className="flex max-h-[min(720px,calc(100vh-48px))] flex-col gap-0 overflow-hidden rounded-[12px] border border-[#ebebeb] bg-white p-0 text-[#171717] sm:max-w-[760px]">
        <div className="grid min-h-0 flex-1 sm:grid-cols-[240px_minmax(0,1fr)]">
          <div className="flex items-center justify-center bg-[#f7f7f7] py-8 sm:py-0">
            {fixedFace ? (
              <ToBossMark size={168} label={label} animated />
            ) : (
              <ToBotMark profile={draft} size={168} label={label} />
            )}
          </div>
          <div className="flex min-h-0 flex-col">
            <div className="shrink-0 px-5 pt-4 pr-14">
              <DialogTitle className="m-0 text-[16px] font-semibold text-[#171717]">
                {t('messages.tobot.edit')}
              </DialogTitle>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">
              <label className="flex flex-col gap-1">
                <span className="text-[12px] text-[#5f5f5f]">{t('messages.tobot.name')}</span>
                <Input
                  value={draft.name}
                  maxLength={32}
                  autoComplete="off"
                  aria-label={t('messages.tobot.name')}
                  placeholder={t(fallbackKey)}
                  onChange={(event) => set({ name: event.target.value })}
                />
              </label>
              <label className="mt-3 flex flex-col gap-1">
                <span className="text-[12px] text-[#5f5f5f]">{t('messages.tobot.personality')}</span>
                <textarea
                  value={draft.personality}
                  maxLength={400}
                  rows={3}
                  aria-label={t('messages.tobot.personality')}
                  placeholder={t('messages.tobot.personality_placeholder')}
                  onChange={(event) => set({ personality: event.target.value })}
                  className="m-0 resize-none rounded-[8px] border border-[#e6e6e6] bg-white px-3 py-2 text-[13px] text-[#171717] outline-none placeholder:text-[#b0b0b0] focus-visible:ring-2 focus-visible:ring-black/10"
                />
              </label>
              {fixedFace ? null : (
                <>
                  <ChoiceGrid label={t('messages.tobot.shape')} columns="grid-cols-4">
                    {TOBOT_SHAPES.map((shape) => (
                      <FaceChoice
                        key={shape}
                        pressed={draft.shape === shape}
                        label={t(`messages.tobot.shape.${shape}`)}
                        dataUi="messages.tobot-shape"
                        dataValue={shape}
                        onChoose={() => set({ shape })}>
                        <ToBotMark profile={{ ...draft, shape }} size={36} />
                      </FaceChoice>
                    ))}
                  </ChoiceGrid>
                  <ChoiceGrid label={t('messages.tobot.expression')} columns="grid-cols-4">
                    {TOBOT_EXPRESSIONS.map((expression) => (
                      <FaceChoice
                        key={expression}
                        pressed={draft.expression === expression}
                        label={t(`messages.tobot.expression.${expression}`)}
                        dataUi="messages.tobot-expression"
                        dataValue={expression}
                        onChoose={() => set({ expression })}>
                        <ToBotMark profile={{ ...draft, expression }} size={36} />
                      </FaceChoice>
                    ))}
                  </ChoiceGrid>
                  <fieldset className="m-0 mt-3 border-0 p-0">
                    <legend className="mb-1.5 text-[12px] text-[#5f5f5f]">{t('messages.tobot.color')}</legend>
                    <div role="radiogroup" aria-label={t('messages.tobot.color')} className="flex flex-wrap gap-2">
                      {TOBOT_COLORS.map((color) => (
                        <button
                          key={color}
                          type="button"
                          role="radio"
                          aria-checked={draft.color === color}
                          aria-label={t(`messages.tobot.color.${color}`)}
                          data-ui="messages.tobot-color"
                          data-value={color}
                          onClick={() => set({ color })}
                          className={cn(
                            'size-7 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-black/20',
                            draft.color === color ? 'ring-2 ring-[#171717] ring-offset-2' : ''
                          )}
                          style={{ backgroundColor: TOBOT_COLOR_HEX[color] }}
                        />
                      ))}
                    </div>
                  </fieldset>
                  <ChoiceGrid label={t('messages.tobot.eyes')} columns="grid-cols-5">
                    {TOBOT_EYES.map((eyes) => (
                      <FaceChoice
                        key={eyes}
                        pressed={draft.eyes === eyes}
                        label={t(`messages.tobot.eyes.${eyes}`)}
                        dataUi="messages.tobot-eyes"
                        dataValue={eyes}
                        onChoose={() => set({ eyes })}>
                        <ToBotMark profile={{ ...draft, eyes }} size={36} />
                      </FaceChoice>
                    ))}
                  </ChoiceGrid>
                </>
              )}
            </div>
            <div className="flex shrink-0 justify-end gap-2 border-t border-[#f0f0f0] px-5 py-3">
              <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
                {t('common.cancel')}
              </Button>
              <Button type="button" data-ui="messages.tobot-save" onClick={save}>
                {t('messages.tobot.save')}
              </Button>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function ChoiceGrid({ label, columns, children }: { label: string; columns: string; children: ReactNode }) {
  return (
    <fieldset className="m-0 mt-3 border-0 p-0">
      <legend className="mb-1.5 text-[12px] text-[#5f5f5f]">{label}</legend>
      <div role="radiogroup" aria-label={label} className={cn('grid gap-1.5', columns)}>
        {children}
      </div>
    </fieldset>
  )
}

function FaceChoice({
  pressed,
  label,
  dataUi,
  dataValue,
  onChoose,
  children
}: {
  pressed: boolean
  label: string
  dataUi: string
  dataValue: string
  onChoose: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={pressed}
      aria-label={label}
      data-ui={dataUi}
      data-value={dataValue}
      onClick={onChoose}
      className={cn(
        'flex flex-col items-center gap-1 rounded-[8px] px-1 py-1.5 text-[11px] text-[#3f3f3f] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10',
        pressed ? 'ring-2 ring-[#171717]' : ''
      )}>
      {children}
      <span className="max-w-full truncate">{label}</span>
    </button>
  )
}
