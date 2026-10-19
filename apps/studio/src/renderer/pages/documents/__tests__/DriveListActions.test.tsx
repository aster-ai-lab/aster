import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { DriveFileCheck, DriveHandoffButton, DriveUploadButton, uploadNameProblem } from '../DriveListActions'

describe('drive list actions', () => {
  it('refuses an empty, hidden, or reserved upload name before any upload', async () => {
    expect(uploadNameProblem('  ')).toBe('empty')
    expect(uploadNameProblem('.secret.txt')).toBe('invalid')
    expect(uploadNameProblem('a/b.txt')).toBe('invalid')
    expect(uploadNameProblem('wiki')).toBe('reserved')
    expect(uploadNameProblem('笔记.txt')).toBeNull()

    const onUpload = vi.fn(async () => {})
    render(<DriveUploadButton onUpload={onUpload} />)
    fireEvent.click(screen.getByRole('button', { name: '上传' }))
    fireEvent.click(screen.getByRole('button', { name: '开始上传' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('名称不能为空。')
    expect(onUpload).not.toHaveBeenCalled()

    fireEvent.change(screen.getByRole('textbox', { name: '上传文件名' }), { target: { value: 'wiki' } })
    fireEvent.click(screen.getByRole('button', { name: '开始上传' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('不能使用百科目录名 wiki。')
    expect(onUpload).not.toHaveBeenCalled()
  })

  it('uploads the typed name and body without a drop', async () => {
    const onUpload = vi.fn(async () => {})
    render(<DriveUploadButton onUpload={onUpload} />)
    fireEvent.click(screen.getByRole('button', { name: '上传' }))
    fireEvent.change(screen.getByRole('textbox', { name: '上传文件名' }), { target: { value: '外来.txt' } })
    fireEvent.change(screen.getByRole('textbox', { name: '上传内容' }), { target: { value: 'hello' } })
    fireEvent.click(screen.getByRole('button', { name: '开始上传' }))
    await vi.waitFor(() => expect(onUpload).toHaveBeenCalledTimes(1))
    const file = onUpload.mock.calls[0][0] as File
    expect(file.name).toBe('外来.txt')
    expect(file.size).toBe(new TextEncoder().encode('hello').length)
    await vi.waitFor(() => expect(screen.queryByRole('textbox', { name: '上传文件名' })).not.toBeInTheDocument())
  })

  it('shows the selected count and hands those names to a task', () => {
    const onHandoff = vi.fn()
    const { rerender } = render(<DriveHandoffButton count={0} names={[]} onHandoff={onHandoff} />)
    expect(screen.getByRole('button', { name: '交给任务' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: '交给任务' }))
    expect(onHandoff).not.toHaveBeenCalled()

    rerender(<DriveHandoffButton count={2} names={['甲.txt', '乙.txt']} onHandoff={onHandoff} />)
    expect(screen.getByText('2 个已选')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '交给任务' }))
    expect(onHandoff).toHaveBeenCalledWith('已交给任务：甲.txt、乙.txt')
    expect(screen.getByRole('status')).toHaveTextContent('已交给任务：甲.txt、乙.txt')
  })

  it('exposes one checkbox per file name', () => {
    const onToggle = vi.fn()
    render(<DriveFileCheck name="甲.txt" checked={false} onToggle={onToggle} />)
    fireEvent.click(screen.getByRole('checkbox', { name: '选择 甲.txt' }))
    expect(onToggle).toHaveBeenCalledTimes(1)
  })
})
