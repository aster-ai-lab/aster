import legalExpertAvatar from '@renderer/assets/agents/legal-expert.jpg'

const AVATARS: Record<string, string> = {
  'legal-expert': legalExpertAvatar
}

const AVATAR_IDS_BY_NAME: Record<string, string> = {
  法务专家: 'legal-expert',
  'a3s-review': 'legal-expert',
  review: 'legal-expert'
}

export function officialAgentAvatar(id: string | undefined) {
  if (!id) return undefined
  const key = AVATARS[id] ? id : AVATAR_IDS_BY_NAME[id.trim()]
  return key ? AVATARS[key] : undefined
}
