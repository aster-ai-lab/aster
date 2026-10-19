import React, { memo } from 'react'

import { ButtonGroup } from '@a3s-lab/ui'

import { modelListClasses } from '../primitives/ProviderSettingsPrimitives'
import { useModelListHealthRun } from './modelListHealthContext'
import ProviderModelAdd from './ProviderModelAdd'
import ProviderModelDownload from './ProviderModelDownload'
import ProviderModelList from './ProviderModelList'
import ProviderModelPullReconcile from './ProviderModelPullReconcile'

interface ModelListProps {
  scrollElement?: HTMLDivElement | null
  providerId: string
  modelPullGuideVersion?: number
  /** Tenant model service: the list is the backend catalog. Hide add, sync, and setup. */
  managed?: boolean
  onContinueApiSetup?: () => void
}

function ModelListContent({
  scrollElement,
  providerId,
  modelPullGuideVersion = 0,
  managed = false,
  onContinueApiSetup
}: {
  scrollElement?: HTMLDivElement | null
  providerId: string
  modelPullGuideVersion?: number
  managed?: boolean
  onContinueApiSetup?: () => void
}) {
  const { isModelChecking } = useModelListHealthRun()
  const disabled = isModelChecking

  return (
    <>
      <ProviderModelList
        scrollElement={scrollElement}
        providerId={providerId}
        disabled={disabled}
        managed={managed}
        onContinueApiSetup={managed ? undefined : onContinueApiSetup}
        actions={
          managed
            ? undefined
            : ({ disabled: toolbarDisabled }) => (
                <ButtonGroup className={modelListClasses.toolbarButtonGroup}>
                  <ProviderModelPullReconcile
                    providerId={providerId}
                    disabled={toolbarDisabled}
                    guideVersion={modelPullGuideVersion}
                  />
                  {providerId === 'ovms' ? (
                    <ProviderModelDownload providerId={providerId} disabled={toolbarDisabled} />
                  ) : (
                    <ProviderModelAdd providerId={providerId} disabled={toolbarDisabled} />
                  )}
                </ButtonGroup>
              )
        }
      />
    </>
  )
}

const ModelList: React.FC<ModelListProps> = ({
  scrollElement,
  providerId,
  modelPullGuideVersion = 0,
  managed = false,
  onContinueApiSetup
}) => {
  return (
    <div className={modelListClasses.cqRoot}>
      <section data-testid="provider-model-list" className={modelListClasses.section}>
        <ModelListContent
          scrollElement={scrollElement}
          providerId={providerId}
          modelPullGuideVersion={modelPullGuideVersion}
          managed={managed}
          onContinueApiSetup={onContinueApiSetup}
        />
      </section>
    </div>
  )
}

export default memo(ModelList)
