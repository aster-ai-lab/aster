import { PERSONAL_CENTER, readCenterId } from '@renderer/data/memberOrgs'
import { ipcApi } from '@renderer/ipc'
import type { AiStreamOpenRequest, AiStreamOpenResponse } from '@shared/ai/transport'

import { withHarnessComponents } from './harnessAssemble'

/** Capture the selected team and recipe before handing the request to transport. */
export async function openStudioStream(request: AiStreamOpenRequest): Promise<AiStreamOpenResponse> {
  if (!request.topicId.startsWith('agent-session:')) return ipcApi.request('ai.stream.open', request)
  const center = readCenterId()
  const captured = {
    ...request,
    ...(request.orgId === undefined && center !== PERSONAL_CENTER ? { orgId: center } : {})
  }
  return ipcApi.request('ai.stream.open', await withHarnessComponents(captured))
}
