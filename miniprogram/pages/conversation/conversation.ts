import { apiRequest } from '../../utils/api'

export {}

interface MessageItem {
  id: string
  sender: 'user' | 'partner' | 'system'
  content: string
  createdAt: string
}

interface ConversationPageData {
  loading: boolean
  hasLoaded: boolean
  error: string
  sending: boolean
  conversationId: string
  partnerName: string
  messages: MessageItem[]
  draft: string
  canSend: boolean
  scrollTarget: string
}

Page<ConversationPageData, WechatMiniprogram.IAnyObject>({
  data: {
    loading: true,
    hasLoaded: false,
    error: '',
    sending: false,
    conversationId: '',
    partnerName: '',
    messages: [],
    draft: '',
    canSend: false,
    scrollTarget: '',
  },
  onLoad(options: Record<string, string | undefined>) {
    const conversationId = options.id || ''
    const partnerName = decodeURIComponent(options.name || '')
    this.setData({ conversationId, partnerName })
    this.loadMessages()
  },
  onShow() {
    if (this.data.conversationId && this.data.hasLoaded) {
      this.loadMessages(true)
    }
  },
  retry() {
    return this.loadMessages()
  },
  async loadMessages(silent = false) {
    if (!this.data.conversationId) {
      this.setData({ loading: false, error: '会话信息不完整，请返回后重试' })
      return
    }
    if (!silent) this.setData({ loading: true, error: '' })
    try {
      const messages = await apiRequest<MessageItem[]>(
        `/api/conversations/${this.data.conversationId}`,
      )
      const formatted = messages.map((message) => ({
        ...message,
        createdAt: formatMessageTime(message.createdAt),
      }))
      this.setData({
        loading: false,
        hasLoaded: true,
        error: '',
        messages: formatted,
        scrollTarget: formatted.length ? `message-${formatted[formatted.length - 1].id}` : '',
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : '会话加载失败'
      if (silent) {
        wx.showToast({ title: message, icon: 'none' })
      } else {
        this.setData({ loading: false, error: message })
      }
    }
  },
  updateDraft(event: WechatMiniprogram.Input) {
    const draft = event.detail.value
    this.setData({ draft, canSend: Boolean(draft.trim()) })
  },
  async sendMessage() {
    const content = this.data.draft.trim()
    if (!content || this.data.sending) return
    this.setData({ sending: true })
    try {
      const message = await apiRequest<MessageItem>(
        `/api/conversations/${this.data.conversationId}`,
        'POST',
        { content },
      )
      const formatted = { ...message, createdAt: formatMessageTime(message.createdAt) }
      this.setData({
        draft: '',
        canSend: false,
        messages: [...this.data.messages, formatted],
        scrollTarget: `message-${message.id}`,
      })
    } catch (error) {
      wx.showToast({ title: error instanceof Error ? error.message : '发送失败', icon: 'none' })
    } finally {
      this.setData({ sending: false })
    }
  },
})

function formatMessageTime(value: string): string {
  const normalized = value.includes('T') ? value : value.replace(' ', 'T') + 'Z'
  const date = new Date(normalized)
  if (Number.isNaN(date.getTime())) return value.slice(0, 16)
  const today = new Date()
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}`
  if (date.toDateString() === today.toDateString()) return time
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${time}`
}

function pad(value: number): string {
  return String(value).padStart(2, '0')
}
