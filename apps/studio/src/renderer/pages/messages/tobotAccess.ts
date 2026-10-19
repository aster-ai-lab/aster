export const TOBOT_PHONE_KEY = 'messages.tobot.phone.v1'
export const TOBOT_WALLET_KEY = 'messages.tobot.wallet.v1'

export type ToBotPhonePolicy = { share: boolean }
export type ToBotWalletPolicy = { view: boolean; cap: number | null }

const CAP_MAX = 1_000_000

export function loadToBotPhone(): ToBotPhonePolicy {
  try {
    const parsed = JSON.parse(localStorage.getItem(TOBOT_PHONE_KEY) ?? '') as { share?: unknown }
    return { share: parsed.share === true }
  } catch {
    return { share: false }
  }
}

export function saveToBotPhone(policy: ToBotPhonePolicy): ToBotPhonePolicy {
  const next = { share: policy.share === true }
  localStorage.setItem(TOBOT_PHONE_KEY, JSON.stringify(next))
  return next
}

function capOf(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > CAP_MAX) return null
  return value
}

export function loadToBotWallet(): ToBotWalletPolicy {
  try {
    const parsed = JSON.parse(localStorage.getItem(TOBOT_WALLET_KEY) ?? '') as { view?: unknown; cap?: unknown }
    return { view: parsed.view === true, cap: capOf(parsed.cap) }
  } catch {
    return { view: false, cap: null }
  }
}

export function saveToBotWallet(policy: ToBotWalletPolicy): ToBotWalletPolicy {
  const next = { view: policy.view === true, cap: capOf(policy.cap) }
  localStorage.setItem(TOBOT_WALLET_KEY, JSON.stringify(next))
  return next
}

function memberPhone(phone: string): string {
  return /^\d{1,20}$/.test(phone) ? phone : ''
}

/** One note for the ToBot turn. There is no call or payment tool behind it. */
export function tobotAccessNote(phone: string): string {
  const listed = memberPhone(phone)
  const phoneLine = loadToBotPhone().share
    ? listed
      ? `The member phone is ${listed}. ToBot may mention it. Do not place a call or send an SMS.`
      : 'The member has no phone on file. Do not invent a phone number, place a call, or send an SMS.'
    : 'ToBot is not allowed to use the member phone. Do not invent a phone number, place a call, or send an SMS.'
  const wallet = loadToBotWallet()
  const walletLine = wallet.view
    ? wallet.cap
      ? `ToBot may discuss the wallet. Do not pay, transfer, or change a balance. Do not propose a payment above ${wallet.cap} yuan.`
      : 'ToBot may discuss the wallet. Do not pay, transfer, or change a balance. Do not propose a payment amount.'
    : 'ToBot is not allowed to manage the wallet. Do not invent a balance, pay, transfer, or change money.'
  return `${phoneLine} ${walletLine}`.slice(0, 480)
}
