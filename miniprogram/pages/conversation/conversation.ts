import { apiRequest } from '../../utils/api'

interface MessageItem {
  id: string
  sender: 'user' | 'partner' | 'system'
  content: string
  createdAt: string
}

Page({
  data: {
    conversationId: '',
    partnerName: '',
    messages: [] as MessageItem[],
    draft: '',
  },
  async onLoad(options: Record<string, string | undefined>) {
    const conversationId = options.id || ''
    const partnerName = decodeURIComponent(options.name || '')
    this.setData({ conversationId, partnerName })
    wx.setNavigationBarTitle({ title: partnerName || '合作会话' })
    await this.loadMessages()
  },
  async loadMessages() {
    try {
      const messages = await apiRequest<MessageItem[]>(
        `/api/conversations/${this.data.conversationId}`,
      )
      this.setData({ messages })
    } catch (error) {
      wx.showToast({ title: error instanceof Error ? error.message : '会话加载失败', icon: 'none' })
    }
  },
  updateDraft(event: WechatMiniprogram.Input) {
    this.setData({ draft: event.detail.value })
  },
  async sendMessage() {
    const content = this.data.draft.trim()
    if (!content) return
    try {
      const message = await apiRequest<MessageItem>(
        `/api/conversations/${this.data.conversationId}`,
        'POST',
        { content },
      )
      this.setData({
        draft: '',
        messages: [...this.data.messages, message],
      })
    } catch (error) {
      wx.showToast({ title: error instanceof Error ? error.message : '发送失败', icon: 'none' })
    }
  },
})
