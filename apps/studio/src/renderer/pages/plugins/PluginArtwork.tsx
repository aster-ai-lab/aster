import type { ReactNode } from 'react'

interface BannerProps {
  title: string
  ribbon?: string
}

function Ribbon({ text }: { text: string }) {
  return (
    <div className="absolute top-3 -left-8 w-36 -rotate-45 bg-[#3dba6c] py-0.5 text-center text-[9px] font-medium tracking-wide text-white">
      {text}
    </div>
  )
}

function AdGuardBanner({ ribbon }: BannerProps) {
  return (
    <div className="relative flex h-full items-center justify-center overflow-hidden bg-[#1c1e22]">
      {ribbon ? <Ribbon text={ribbon} /> : null}
      <div className="flex items-center gap-2.5">
        <svg width="28" height="28" viewBox="0 0 28 28" aria-hidden="true">
          <path d="M14 2.5 23.5 6.2v7.6c0 5.4-3.7 9.3-9.5 11.7-5.8-2.4-9.5-6.3-9.5-11.7V6.2L14 2.5Z" fill="#3dba6c" />
          <path d="m9.2 14.1 3.1 3.1 6.6-6.8" fill="none" stroke="#fff" strokeWidth="2" strokeLinecap="round" />
        </svg>
        <span className="text-[22px] font-semibold tracking-tight text-white">adguard</span>
      </div>
    </div>
  )
}

function ImmersiveBanner({ title }: BannerProps) {
  return (
    <div className="flex h-full items-center gap-3 bg-gradient-to-br from-[#fdeff3] to-[#f7fbff] px-5">
      <svg width="48" height="48" viewBox="0 0 48 48" aria-hidden="true" className="shrink-0">
        <defs>
          <linearGradient id="plugin-immersive-mark" x1="8" y1="6" x2="40" y2="42" gradientUnits="userSpaceOnUse">
            <stop stopColor="#ff7a90" />
            <stop offset="1" stopColor="#7aa2ff" />
          </linearGradient>
        </defs>
        <rect x="2" y="2" width="44" height="44" rx="10" fill="url(#plugin-immersive-mark)" />
        <text x="24" y="30" textAnchor="middle" fill="#fff" fontSize="16" fontWeight="600">
          中A
        </text>
      </svg>
      <div className="min-w-0">
        <div className="truncate text-[15px] font-semibold text-[#1a1a1a]">{title}</div>
        <div className="text-[11px] text-[#8d8d8d]">Immersive Translate</div>
      </div>
    </div>
  )
}

function TampermonkeyBanner() {
  return (
    <div className="flex h-full items-center justify-center bg-[#f6f6f7]">
      <div className="flex size-16 items-center justify-center gap-1.5 rounded-[18px] bg-[#111]">
        <span className="size-3.5 rounded-full bg-white" />
        <span className="size-3.5 rounded-full bg-white" />
      </div>
    </div>
  )
}

function GlobalSpeedBanner() {
  return (
    <div className="relative flex h-full flex-col justify-center overflow-hidden bg-[#101114] px-4">
      <div className="flex items-center gap-2">
        <svg width="28" height="28" viewBox="0 0 28 28" aria-hidden="true">
          <path d="M8 6.5v15l14-7.5L8 6.5Z" fill="#2f80ff" />
        </svg>
        <span className="text-[26px] font-bold tracking-tight text-[#3ec6ff]">2x</span>
      </div>
      <div className="mt-1 text-[13px] font-semibold tracking-wide text-[#3ec6ff]">GLOBAL SPEED</div>
      <div className="mt-2 w-fit max-w-full rounded-[4px] bg-[#b6ffe4] px-1.5 py-0.5 text-[9px] font-semibold whitespace-nowrap text-[#123]">
        UNIVERSAL VIDEO SPEED
      </div>
    </div>
  )
}

function RightClickBanner() {
  return (
    <div className="flex h-full flex-col items-center justify-center bg-white">
      <svg width="54" height="54" viewBox="0 0 54 54" aria-hidden="true">
        <path d="M27 6a21 21 0 1 1-18.2 10.5L27 27V6Z" fill="#f5b423" />
        <path d="M27 6a21 21 0 0 0-18.2 10.5" fill="none" stroke="#f5b423" strokeWidth="3" />
        <circle cx="40" cy="12" r="2" fill="#f5b423" />
        <circle cx="46" cy="18" r="1.4" fill="#f5b423" />
      </svg>
      <div className="mt-1 text-[13px] font-semibold text-[#1a1a1a]">Re-enable</div>
      <div className="text-[11px] text-[#666]">Right-Click Context Menu</div>
    </div>
  )
}

function ScriptCatBanner() {
  return (
    <div className="flex h-full items-center justify-center bg-white">
      <svg width="72" height="56" viewBox="0 0 72 56" aria-hidden="true">
        <path d="M16 28 8 10l14 8 14-12 14 12 14-8-8 18c4 4 6 10 6 16H10c0-6 2-12 6-16Z" fill="#2f7bff" />
        <circle cx="28" cy="32" r="2.2" fill="#fff" />
        <circle cx="44" cy="32" r="2.2" fill="#fff" />
        <path d="M32 40c2.2 2 5.8 2 8 0" fill="none" stroke="#fff" strokeWidth="1.8" strokeLinecap="round" />
      </svg>
    </div>
  )
}

function CoCoCutBanner() {
  return (
    <div className="flex h-full items-center justify-center bg-[#eef3f8] px-4">
      <div className="w-full max-w-[210px] overflow-hidden rounded-[8px] border border-[#d7e0ea] bg-white shadow-[0_4px_12px_rgba(30,50,80,0.06)]">
        <div className="flex h-5 items-center gap-1 bg-[#f4f6f8] px-2">
          <span className="size-1.5 rounded-full bg-[#e2e6ea]" />
          <span className="size-1.5 rounded-full bg-[#e2e6ea]" />
          <span className="size-1.5 rounded-full bg-[#e2e6ea]" />
        </div>
        <div className="flex items-end gap-2 px-3 py-3">
          <div className="h-8 flex-1 rounded-[4px] bg-[#d9e6f5]" />
          <div className="flex flex-col gap-1 text-[9px] font-semibold tracking-wide text-[#5d7394]">
            <span>HLS</span>
            <span>M3U8</span>
            <span className="text-[#e25b5b]">LIVE</span>
          </div>
        </div>
      </div>
    </div>
  )
}

function ScrnliBanner() {
  return (
    <div className="flex h-full flex-col items-center justify-center bg-gradient-to-br from-[#2f6bff] to-[#3b82ff]">
      <div className="flex size-12 items-center justify-center rounded-[14px] bg-white/15">
        <svg width="28" height="28" viewBox="0 0 28 28" aria-hidden="true">
          <rect x="4" y="7" width="20" height="15" rx="4" fill="none" stroke="#fff" strokeWidth="1.8" />
          <circle cx="14" cy="14.5" r="3.2" fill="none" stroke="#fff" strokeWidth="1.8" />
          <circle cx="14" cy="14.5" r="1.2" fill="#ffb020" />
        </svg>
      </div>
      <div className="mt-1.5 text-[18px] font-semibold tracking-tight text-white">scrnli</div>
      <div className="mt-1 text-[8px] font-medium whitespace-nowrap text-white/80">SCREENSHOT · SCREEN RECORD</div>
    </div>
  )
}

function ClipperBanner() {
  return (
    <div className="flex h-full items-center justify-center bg-[#f4f7fb]">
      <div className="w-28 rounded-[8px] border border-[#e4e9f0] bg-white p-2 shadow-sm">
        <div className="h-2 w-16 rounded-full bg-[#d5deea]" />
        <div className="mt-1.5 h-1.5 w-full rounded-full bg-[#eef2f6]" />
        <div className="mt-1 h-1.5 w-4/5 rounded-full bg-[#eef2f6]" />
        <div className="mt-2 h-6 rounded-[4px] bg-[#dbeafe]" />
      </div>
    </div>
  )
}

function JsonViewBanner() {
  return (
    <div className="flex h-full items-center justify-center bg-[#16181d] font-mono text-[11px] leading-4 text-[#9ad7ff]">
      <pre className="m-0">{`{\n  "ok": true\n}`}</pre>
    </div>
  )
}

function SubsBanner() {
  return (
    <div className="flex h-full flex-col items-center justify-end bg-[#1b2430] pb-4">
      <div className="mb-2 h-10 w-24 rounded-[6px] bg-[#2c3a4d]" />
      <div className="rounded-full bg-black/50 px-2 py-0.5 text-[11px] text-white">Hello · Hi</div>
    </div>
  )
}

function FocusBanner() {
  return (
    <div className="flex h-full items-center justify-center bg-[#fff8ef]">
      <div className="flex size-16 items-center justify-center rounded-full border-[6px] border-[#f0d3a4]">
        <div className="h-5 w-0.5 origin-bottom -translate-y-1 rotate-45 bg-[#c47b2b]" />
      </div>
    </div>
  )
}

const BANNERS: Record<string, (props: BannerProps) => ReactNode> = {
  adguard: AdGuardBanner,
  immersive: ImmersiveBanner,
  tampermonkey: () => <TampermonkeyBanner />,
  globalspeed: () => <GlobalSpeedBanner />,
  rightclick: () => <RightClickBanner />,
  scriptcat: () => <ScriptCatBanner />,
  cococut: () => <CoCoCutBanner />,
  scrnli: () => <ScrnliBanner />,
  clipper: () => <ClipperBanner />,
  jsonview: () => <JsonViewBanner />,
  subs: () => <SubsBanner />,
  focus: () => <FocusBanner />
}

export function hasPluginBanner(id: string): boolean {
  return Object.hasOwn(BANNERS, id)
}

export function PluginBanner({ id, title, ribbon }: { id: string; title: string; ribbon?: string }) {
  const Banner = BANNERS[id]
  if (!Banner) return <div className="h-full bg-[#f4f4f5]" />
  return <div className="h-full">{Banner({ title, ribbon })}</div>
}

export function HeroCollage({
  sync,
  read,
  watch,
  note
}: {
  sync: string
  read: string
  watch: string
  note: string
}) {
  return (
    <div className="pointer-events-none absolute inset-y-0 right-0 hidden w-[500px] sm:block">
      <div className="absolute -top-6 right-1 size-36 rounded-full bg-[#b7d4fb]" />
      <svg
        className="absolute top-3 right-8 size-14 text-white"
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true">
        <path
          d="M4.5 12.5 9.2 17.5 19.5 6.5"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      <div className="absolute top-2.5 right-[200px] w-[200px] -rotate-3 rounded-[12px] border border-white/80 bg-white/95 p-2.5 shadow-[0_8px_18px_rgba(40,80,140,0.08)]">
        <div className="text-[11px] leading-4 text-[#7aa2d6]">{sync}</div>
        <div className="mt-1.5 h-7 rounded-[6px] bg-[#eef3f8]" />
      </div>
      <div className="absolute top-[54px] right-7 w-[220px] rounded-[14px] bg-white p-3 shadow-[0_12px_28px_rgba(40,80,140,0.14)]">
        <div className="text-[13px] leading-5 font-medium text-[#1a1a1a]">{read}</div>
        <div className="text-[13px] leading-5 font-medium text-[#1a1a1a]">{watch}</div>
        <div className="mt-0.5 truncate text-[10px] leading-4 text-[#9aa3ad]">{note}</div>
        <div className="mt-1.5 flex h-8 overflow-hidden rounded-[6px] bg-[#1c1c1c]">
          <div className="m-1.5 w-11 rounded-[3px] bg-[#3a3a3a]" />
          <div className="my-1.5 mr-1.5 flex-1 rounded-[3px] bg-[#2a2a2a]" />
        </div>
      </div>
    </div>
  )
}
