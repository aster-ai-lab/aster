import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input
} from '@a3s-lab/ui'
import { X } from 'lucide-react'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { workspaceEntryName } from './workbenchWorkspace'

const PROJECT_NAME_MAX_LENGTH = 64

type Props = {
  open: boolean
  existingNames: readonly string[]
  onOpenChange: (open: boolean) => void
  /** Rejects when the project could not be created; the dialog then stays open with the draft. */
  onCreate?: (name: string) => Promise<void> | void
}

export function WorkbenchProjectCreateDialog({ open, existingNames, onOpenChange, onCreate }: Props) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [creating, setCreating] = useState(false)
  const inputId = useId()
  const hintId = useId()

  const trimmed = name.trim()
  const taken = existingNames.some((existing) => existing.trim().toLowerCase() === trimmed.toLowerCase())
  const error = !trimmed
    ? null
    : !workspaceEntryName(name)
      ? t('workbench.projects.create_invalid_chars')
      : taken
        ? t('workbench.projects.create_duplicate')
        : null
  const canSubmit = Boolean(trimmed) && !error && Boolean(onCreate) && !creating

  const changeOpen = (next: boolean) => {
    if (creating) return
    if (!next) setName('')
    onOpenChange(next)
  }

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!canSubmit || !onCreate) return
    setCreating(true)
    try {
      await onCreate(trimmed)
      setName('')
      onOpenChange(false)
    } catch {
      // The caller reports the failure. Keep the draft so the name can be retried.
    } finally {
      setCreating(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={changeOpen}>
      <DialogContent
        data-ui="workbench.projects.create"
        motion="fade-scale"
        showCloseButton={false}
        closeOnOverlayClick={!creating}
        overlayClassName="z-[220]"
        onEscapeKeyDown={(event) => {
          if (creating) event.preventDefault()
        }}
        className="z-[220] w-[320px] sm:max-w-[320px]">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          disabled={creating}
          aria-label={t('common.close')}
          onClick={() => changeOpen(false)}
          className="absolute top-6 right-6 rounded-[6px] text-[#3f3f3f] hover:bg-[#f5f5f5]">
          <X size={14} strokeWidth={1.7} />
        </Button>
        <DialogHeader className="gap-1 text-left">
          <DialogTitle className="pr-10 text-[15px] leading-[1.4] font-medium text-[#171717]">
            {t('workbench.projects.create')}
          </DialogTitle>
          <DialogDescription className="text-[13px] leading-[1.5] text-[#8a8a8a]">
            {t('workbench.projects.create_description')}
          </DialogDescription>
        </DialogHeader>

        <form className="flex flex-col gap-4" noValidate onSubmit={(event) => void submit(event)}>
          <div className="flex flex-col gap-1.5">
            <label htmlFor={inputId} className="text-[13px] font-medium text-[#171717]">
              {t('workbench.projects.create_name')}
            </label>
            <Input
              id={inputId}
              value={name}
              autoFocus
              autoComplete="off"
              spellCheck={false}
              maxLength={PROJECT_NAME_MAX_LENGTH}
              disabled={creating}
              placeholder={t('workbench.projects.create_placeholder')}
              aria-invalid={error ? true : undefined}
              aria-describedby={error ? hintId : undefined}
              onChange={(event) => setName(event.target.value)}
            />
            {error && (
              <p id={hintId} role="alert" className="text-[12px] leading-[1.4] text-[#b42318]">
                {error}
              </p>
            )}
          </div>

          <DialogFooter className="flex-row justify-end gap-2">
            <Button type="button" variant="outline" size="md" disabled={creating} onClick={() => changeOpen(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" variant="emphasis" size="md" loading={creating} disabled={!canSubmit}>
              {creating ? t('workbench.projects.creating') : t('workbench.projects.create_confirm')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
