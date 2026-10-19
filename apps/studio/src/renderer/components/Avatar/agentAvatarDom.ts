import { capabilityIconDataUrl } from '@renderer/services/capabilityIcon'
import { DEFAULT_AGENT_AVATAR } from '@renderer/utils/agent'

import { agentAvatarInitial, readAgentAvatar } from './agentAvatarValue'

const MARK_STYLE =
  'display:inline-flex;align-items:center;justify-content:center;width:14px;height:14px;margin:0 4px 0 2px;border-radius:4px;overflow:hidden;font-size:12px;line-height:1;vertical-align:-2px;'

function cssUrl(src: string) {
  return `url("${src.replace(/["\\\n\r]/g, '')}")`
}

function paintImage(mark: HTMLElement, src: string) {
  mark.textContent = ''
  mark.style.backgroundImage = cssUrl(src)
}

/**
 * The avatar mark for a non-React surface such as an editor decoration, where an
 * `<img>` draws as a broken-image glyph. Images paint as a background.
 */
export function agentAvatarElement(avatar: string | null | undefined, name?: string): HTMLElement {
  const mark = document.createElement('span')
  mark.setAttribute('contenteditable', 'false')
  mark.setAttribute('aria-hidden', 'true')
  mark.style.cssText = `${MARK_STYLE}background-size:cover;background-position:center;`
  const value = readAgentAvatar(avatar)
  mark.setAttribute('data-avatar', value?.kind ?? 'fallback')
  if (value?.kind === 'image') {
    mark.style.backgroundColor = '#f2f3f5'
    paintImage(mark, value.src)
    return mark
  }
  const fallback = value?.kind === 'glyph' ? value.glyph : agentAvatarInitial(name) || DEFAULT_AGENT_AVATAR
  mark.textContent = fallback
  if (value?.kind === 'catalog-icon') {
    void capabilityIconDataUrl(value.path).then((src) => {
      if (src) paintImage(mark, src)
    })
  }
  return mark
}
