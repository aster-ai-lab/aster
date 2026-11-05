import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, type ReactNode, useState } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { Popover } from '../src/internal/design-system/primitives/overlay/popover'

let root: Root | undefined

beforeEach(() => {
  document.documentElement.removeAttribute('data-motion-paused')
})

afterEach(async () => {
  await act(async () => root?.unmount())
  root = undefined
  document.body.replaceChildren()
})

function Harness({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  return (
    <Popover
      label="More"
      panelLabel="Actions"
      open={open}
      onOpenChange={onOpenChange}
      trigger={(props) => <button {...props}>More</button>}>
      <button type="button">Rename</button>
    </Popover>
  )
}

async function mount(node: ReactNode) {
  const host = document.createElement('div')
  document.body.append(host)
  root = createRoot(host)
  await act(async () => root!.render(node))
}

describe('Office popover exit presence', () => {
  it('keeps the panel mounted with data-state=closed through the shared 200ms exit', async () => {
    let open = true
    let setOpen: (next: boolean) => void = () => undefined
    function Controlled() {
      const [value, update] = useState(true)
      open = value
      setOpen = update
      return <Harness open={value} onOpenChange={update} />
    }
    await mount(<Controlled />)
    const panel = () => document.querySelector<HTMLElement>('.ds-popover-panel')
    expect(panel()?.getAttribute('data-state')).toBe('open')
    await act(async () => setOpen(false))
    expect(open).toBe(false)
    expect(panel()?.getAttribute('data-state')).toBe('closed')
    expect(panel()?.hasAttribute('inert')).toBe(true)
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 220))
    })
    expect(panel()).toBeNull()
  })

  it('removes the panel immediately when reduced motion is preferred', async () => {
    const media = window.matchMedia
    window.matchMedia = ((query: string) =>
      ({
        matches: String(query).includes('prefers-reduced-motion'),
        media: query,
        onchange: null,
        addListener() {},
        removeListener() {},
        addEventListener() {},
        removeEventListener() {},
        dispatchEvent() {
          return false
        },
      })) as typeof window.matchMedia
    try {
      let setOpen: (next: boolean) => void = () => undefined
      function Controlled() {
        const [value, update] = useState(true)
        setOpen = update
        return <Harness open={value} onOpenChange={update} />
      }
      await mount(<Controlled />)
      expect(document.querySelector('.ds-popover-panel')).not.toBeNull()
      await act(async () => setOpen(false))
      expect(document.querySelector('.ds-popover-panel')).toBeNull()
    } finally {
      window.matchMedia = media
    }
  })
})
