import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

const search = vi.hoisted(() => vi.fn())
const openTab = vi.hoisted(() => vi.fn())

vi.mock('@renderer/hooks/tab', () => ({ useTabs: () => ({ openTab }) }))

vi.mock('@renderer/drive/encyclopediaClient', async (original) => ({
  ...(await original<typeof import('@renderer/drive/encyclopediaClient')>()),
  searchTeamPassages: search
}))

import { parseTeamPassage } from '@renderer/drive/encyclopediaClient'

import { TeamPassageSearch } from '../TeamPassageSearch'

const FILE = '6f1c1f1e-0000-4000-8000-0000000000aa'

describe('team passage search', () => {
  afterEach(() => {
    cleanup()
    search.mockReset()
  })

  it('lists visible passages with a link to the drive file and the entity path', async () => {
    search.mockResolvedValue([
      {
        passageId: 'p1',
        fileId: FILE,
        heading: '星河项目云资源',
        folderName: '',
        body: '星河项目的全部服务运行在阿里云华东二可用区。',
        path: ['张三', '星河项目']
      }
    ])
    render(<TeamPassageSearch />)
    const input = screen.getByRole('searchbox')
    fireEvent.change(input, { target: { value: '张三那边的系统部署在哪家云上' } })
    fireEvent.submit(input.closest('form') as HTMLFormElement)
    expect(search).toHaveBeenCalledWith('张三那边的系统部署在哪家云上', expect.any(AbortSignal))
    const hit = await screen.findByText('星河项目云资源')
    const row = hit.closest('[data-ui="knowledge.team-search.hit"]') as HTMLElement
    const open = row.querySelector('[data-ui="knowledge.team-search.open"]') as HTMLElement
    expect(open).toHaveAttribute('href', `/app/files?driveFile=${FILE}`)
    expect(row).toHaveTextContent('张三 → 星河项目')
    // Tab routers use memory history: a page load would land on "not found".
    expect(fireEvent.click(open)).toBe(false)
    expect(openTab).toHaveBeenCalledWith(`/app/files?driveFile=${FILE}`)
  })

  it('says nothing was found without leaking why', async () => {
    search.mockResolvedValue([])
    render(<TeamPassageSearch />)
    const input = screen.getByRole('searchbox')
    fireEvent.change(input, { target: { value: '基本户开户行' } })
    fireEvent.submit(input.closest('form') as HTMLFormElement)
    await waitFor(() => expect(document.querySelector('[data-ui="knowledge.team-search.empty"]')).not.toBeNull())
    fireEvent.keyDown(input, { key: 'Escape' })
    expect(document.querySelector('[data-ui="knowledge.team-search.results"]')).toBeNull()
  })

  it('parses only well-formed team passage references', () => {
    expect(parseTeamPassage(`team/${FILE}/${FILE}@3`)).toEqual({ fileId: FILE, passageId: FILE, generation: 3 })
    expect(parseTeamPassage(`team/${FILE}/../etc@3`)).toBeNull()
    expect(parseTeamPassage('local/偏好/食谱.md')).toBeNull()
  })
})
