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
    loading: true,
    refreshing: false,
    markingRead: false,
    error: '',
    hasLoaded: false,
    hasUnread: false,
    notices: [] as Notice[],
    chats: [] as Chat[],
  },
  lifetimes: {
    async attached() {
      await this.loadMessages()
    },
  },
  pageLifetimes: {
    async show() {
      if (this.data.hasLoaded && !this.data.refreshing) {
        await this.loadMessages(false)
      }
    },
  },
  methods: {
    async loadMessages(initial = true) {
      this.setData(initial ? { loading: true, error: '' } : { refreshing: true })
      try {
        const response = await apiRequest<MessagesResponse>('/api/messages')
        this.setData({
          ...response,
          loading: false,
          refreshing: false,
          hasLoaded: true,
          hasUnread: response.notices.some((notice) => !notice.isRead),
        })
      } catch (error) {
        const message = error instanceof Error ? error.message : '消息加载失败'
        if (initial) {
          this.setData({ loading: false, error: message })
        } else {
          this.setData({ refreshing: false })
          wx.showToast({ title: message, icon: 'none' })
        }
      }
    },
    async markAllRead() {
      if (!this.data.hasUnread || this.data.markingRead) return
      this.setData({ markingRead: true })
      try {
        await apiRequest('/api/messages/read-all', 'POST')
        this.setData({
          notices: this.data.notices.map((notice) => ({ ...notice, isRead: true })),
          hasUnread: false,
        })
        wx.showToast({ title: '已全部标记为已读', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '操作失败', icon: 'none' })
      } finally {
        this.setData({ markingRead: false })
      }
    },
    openChat(event: WechatMiniprogram.TouchEvent) {
      const id = event.currentTarget.dataset.id as string
      const name = event.currentTarget.dataset.name as string
      this.setData({
        chats: this.data.chats.map((chat) => (chat.id === id ? { ...chat, unread: 0 } : chat)),
      })
      wx.navigateTo({
        url: `/pages/conversation/conversation?id=${encodeURIComponent(id)}&name=${encodeURIComponent(name)}`,
      })
    },
    retry() {
      return this.loadMessages(true)
    },
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
  },
})
