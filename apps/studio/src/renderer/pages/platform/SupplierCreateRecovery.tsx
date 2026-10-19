import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@a3s-lab/ui'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { MemberOrgError } from '@renderer/data/memberOrgs'
import { resolveSupplierCreate, type SupplierCreateResolution } from '@renderer/data/platformSuppliers'

export function SupplierCreateRecovery({
  intent,
  onBusy,
  onResolved,
  onUnavailable,
  onClose,
  onView,
  onRestoreFocus
}: {
  intent: string
  onBusy: (busy: boolean) => void
  onResolved: (outcome: SupplierCreateResolution) => boolean
  onUnavailable: () => void
  onClose: () => void
  onView: (id: string) => void
  onRestoreFocus: () => void
}) {
  const { t } = useTranslation()
  const alive = useRef(true)
  const gate = useRef(false)
  const cancel = useRef<HTMLButtonElement>(null)
  const [pending, setPending] = useState(false)
  const [outcome, setOutcome] = useState<SupplierCreateResolution>()
  const [released, setReleased] = useState(false)
  const [error, setError] = useState('')
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])
  const resolve = async () => {
    if (gate.current || released) return
    gate.current = true
    setPending(true)
    onBusy(true)
    setError('')
    try {
      const result = await resolveSupplierCreate(intent)
      if (!alive.current) return
      setOutcome(result)
      const cleared = onResolved(result)
      setReleased(cleared)
      if (!cleared) setError('platform.supplierForm.recoveryStorageFailed')
    } catch (error) {
      if (!alive.current) return
      if (error instanceof MemberOrgError && [401, 404].includes(error.status ?? 0)) onUnavailable()
      else setError('platform.supplierForm.recoveryFailed')
    } finally {
      if (alive.current) {
        gate.current = false
        setPending(false)
        onBusy(false)
      }
    }
  }
  const close = () => {
    if (!gate.current) onClose()
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) close()
      }}>
      <DialogContent
        className="max-h-[90dvh] overflow-y-auto [&_:focus-visible]:outline-2 [&_:focus-visible]:outline-offset-2 [&_:focus-visible]:outline-[#171717]"
        closeLabel={t('common.close')}
        showCloseButton={!pending}
        closeOnOverlayClick={!pending}
        onOpenAutoFocus={(event) => {
          event.preventDefault()
          cancel.current?.focus()
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault()
          onRestoreFocus()
        }}
        onEscapeKeyDown={(event) => {
          if (pending) event.preventDefault()
        }}>
        <DialogHeader>
          <DialogTitle>{t('platform.supplierForm.recoveryTitle')}</DialogTitle>
          <DialogDescription>{t('platform.supplierForm.recoveryDescription')}</DialogDescription>
        </DialogHeader>
        <p className="m-0 text-[12px] break-all text-[#3f3f3f]">
          {t('platform.supplierForm.recoveryIntent', { intent })}
        </p>
        {pending ? <p role="status">{t('platform.supplierForm.submitting')}</p> : null}
        {error ? <p role="alert">{t(error)}</p> : null}
        {outcome ? (
          <p role="status" className="m-0 text-[13px] break-words text-[#171717]">
            {outcome.state === 'created'
              ? t('platform.supplierForm.recoveryCreated', { id: outcome.id, revision: outcome.revision })
              : t('platform.supplierForm.recoveryCanceled')}
          </p>
        ) : null}
        <DialogFooter>
          <Button ref={cancel} variant="outline" disabled={pending} onClick={close}>
            {t(outcome ? 'common.close' : 'common.cancel')}
          </Button>
          {outcome?.state === 'created' ? (
            <Button variant="outline" disabled={pending} onClick={() => onView(outcome.id)}>
              {t('platform.supplierForm.recoveryView')}
            </Button>
          ) : null}
          {!released ? (
            <Button disabled={pending} onClick={() => void resolve()}>
              {t(
                pending
                  ? 'platform.supplierForm.submitting'
                  : error
                    ? 'common.retry'
                    : 'platform.supplierForm.recoveryConfirm'
              )}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
