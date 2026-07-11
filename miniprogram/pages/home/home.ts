import { apiRequest } from '../../utils/api'

export {}

interface Metric {
  value: string
  label: string
}

interface Recommendation {
  id: string
  avatar: string
  avatarClass: string
  title: string
  subtitle: string
  score: number
  price: string
}

interface HomeResponse {
  headerSubtitle: string
  name: string
  metrics: Metric[]
  recommendations: Recommendation[]
}

Component({
  data: {
    loading: true,
    error: '',
    greeting: '你好',
    todayLabel: '',
    headerSubtitle: '',
    name: '',
    metrics: [] as Metric[],
    recommendations: [] as Recommendation[],
    connectingId: '',
  },
  lifetimes: {
    async attached() {
      const now = new Date()
      this.setData({
        greeting: greetingForHour(now.getHours()),
        todayLabel: formatToday(now),
      })
      await this.loadHome()
    },
  },
  methods: {
    async loadHome() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<HomeResponse>('/api/home')
        this.setData({ ...response, loading: false })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '首页加载失败',
        })
      }
    },
    openAI() {
      wx.redirectTo({ url: '/pages/ai/ai' })
    },
    openMatch() {
      wx.redirectTo({ url: '/pages/match/match' })
    },
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
    openProfile() {
      wx.redirectTo({ url: '/pages/profile/profile' })
    },
    async contactPartner(event: WechatMiniprogram.TouchEvent) {
      const partnerId = event.currentTarget.dataset.id as string
      if (this.data.connectingId) return
      this.setData({ connectingId: partnerId })
      try {
        const response = await apiRequest<{ conversationId: string; partnerName: string }>(
          '/api/plaza/connect',
          'POST',
          { partnerId },
        )
        wx.navigateTo({
          url: `/pages/conversation/conversation?id=${encodeURIComponent(response.conversationId)}&name=${encodeURIComponent(response.partnerName)}`,
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '发起沟通失败', icon: 'none' })
      } finally {
        this.setData({ connectingId: '' })
      }
    },
  },
})

function greetingForHour(hour: number): string {
  if (hour < 6) return '夜深了'
  if (hour < 11) return '早上好'
  if (hour < 14) return '中午好'
  if (hour < 19) return '下午好'
  return '晚上好'
}

function formatToday(date: Date): string {
  const weekdays = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
  return `${date.getMonth() + 1}月${date.getDate()}日 · ${weekdays[date.getDay()]}`
}
