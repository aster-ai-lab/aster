import { Button, getToastUtilities, ToastViewport } from '@a3s-lab/ui'
import type { Meta, StoryObj } from '@storybook/react'

const toast = getToastUtilities()

const meta: Meta<typeof ToastViewport> = {
  title: 'Components/Primitives/Toast',
  component: ToastViewport,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'A shared toast viewport and utility API for app-wide feedback messages.'
      }
    }
  },
  tags: ['autodocs']
}

export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-2">
      <ToastViewport />
      <Button onClick={() => toast.success('Settings saved')}>Success</Button>
      <Button variant="outline" onClick={() => toast.info('Sync started')}>
        Info
      </Button>
      <Button variant="outline" onClick={() => toast.warning('Storage is almost full')}>
        Warning
      </Button>
      <Button variant="destructive" onClick={() => toast.error('Upload failed')}>
        Error
      </Button>
    </div>
  )
}

export const WithDescription: Story = {
  render: () => (
    <div className="flex items-center gap-2">
      <ToastViewport />
      <Button
        onClick={() =>
          toast.success({
            description: 'Your changes are now available across open windows.',
            title: 'Profile updated'
          })
        }>
        Show toast
      </Button>
    </div>
  )
}

export const WithAction: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-2">
      <ToastViewport />
      <Button
        onClick={() =>
          toast.success({
            action: {
              label: 'Undo',
              onClick: () => {
                toast.success('Item restored')
              }
            },
            title: 'Item moved to Recycle Bin'
          })
        }>
        Title only
      </Button>
      <Button
        variant="outline"
        onClick={() =>
          toast.info({
            action: {
              label: 'Undo',
              onClick: () => {
                toast.success('Item restored')
              }
            },
            description: 'The item can be restored for 30 days.',
            title: 'Item moved to Recycle Bin'
          })
        }>
        With description
      </Button>
    </div>
  )
}

export const Loading: Story = {
  render: () => (
    <div className="flex items-center gap-2">
      <ToastViewport />
      <Button
        onClick={() =>
          toast.loading({
            promise: new Promise((resolve) => setTimeout(resolve, 1200)),
            title: 'Generating summary'
          })
        }>
        Start loading
      </Button>
    </div>
  )
}

export const Compact: Story = {
  ...Basic,
  play: () => {
    toast.success({ title: '你的软件已是最新版本', timeout: 0 })
  }
}

export const LongMessage: Story = {
  ...Basic,
  play: () => {
    toast.info({
      title: '文档已保存，较长的提示会自然换行，完整展示这次操作的结果',
      description: '你可以继续处理其他任务，也可以查看刚刚保存的文档。',
      action: {
        label: '查看',
        onClick: () => {
          toast.success('已打开文档')
        }
      },
      timeout: 0
    })
  }
}
