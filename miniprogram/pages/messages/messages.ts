import { apiRequest } from '../../utils/api'

export {}

interface Notice {
  id: string
  icon: string
  iconClass: string
  title: string
  desc: string
  time: string
  isRead: boolean
}

interface Chat {
  id: string
  avatar: string
  avatarClass: string
  name: string
  message: string
  time: string
  unread: number
}

interface MessagesResponse {
  notices: Notice[]
  chats: Chat[]
}

Component({
  data: {
    notices: [] as Notice[],
    chats: [] as Chat[],
  },
  lifetimes: {
    async attached() {
      await this.loadMessages()
    },
  },
  methods: {
    async loadMessages() {
      try {
        const response = await apiRequest<MessagesResponse>('/api/messages')
        this.setData(response)
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '消息加载失败', icon: 'none' })
      }
    },
    async markAllRead() {
      try {
        await apiRequest('/api/messages/read-all', 'POST')
        this.setData({
          notices: this.data.notices.map((notice) => ({ ...notice, isRead: true })),
        })
        wx.showToast({ title: '已全部标记为已读', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '操作失败', icon: 'none' })
      }
    },
    openChat(event: WechatMiniprogram.TouchEvent) {
      const id = event.currentTarget.dataset.id as string
      const name = event.currentTarget.dataset.name as string
      wx.navigateTo({
        url: `/pages/conversation/conversation?id=${encodeURIComponent(id)}&name=${encodeURIComponent(name)}`,
      })
    },
  },
})
