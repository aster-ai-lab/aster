import { prepareBuiltinSkill } from '@renderer/pages/capabilities/capabilitiesData'

import { skillPromptText } from './homeSlashCatalog'
import { queueWorkbenchComposer } from './workbenchComposerBridge'

/** Hand the home composer the built-in skill that can create a scheduled task. */
export function queueScheduledTaskComposer(translate: (key: string) => string) {
  void prepareBuiltinSkill('schedule-task').catch(() => undefined)
  queueWorkbenchComposer(`${skillPromptText('schedule-task')}${translate('settings.scheduledTasks.agentCreatePrompt')}`)
}
