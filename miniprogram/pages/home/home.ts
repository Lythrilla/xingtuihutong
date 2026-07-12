import { apiRequest } from '../../utils/api'

export {}

interface Recommendation {
  id: string
  avatar: string
  avatarClass: string
  verified: boolean
  preferred: boolean
  title: string
  subtitle: string
  score: string
  price: string
}

interface HomeResponse {
  name: string
  recommendations: Recommendation[]
}

Component({
  data: {
    loading: true,
    error: '',
    greeting: '早上好',
    statusBarHeight: 20,
    name: '',
    recommendations: [] as Recommendation[],
    connectingId: '',
  },
  lifetimes: {
    attached() {
      const systemInfo = wx.getSystemInfoSync()
      this.setData({
        greeting: greetingForHour(new Date().getHours()),
        statusBarHeight: systemInfo.statusBarHeight || 20,
      })
      this.loadHome()
    },
  },
  methods: {
    async loadHome() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<HomeResponse>('/api/home')
        const recommendations = (response.recommendations || []).map((item) => {
          const hasImageAvatar = item.avatar && (item.avatar.indexOf('http') === 0 || item.avatar.indexOf('/') === 0)
          return {
            ...item,
            avatar: hasImageAvatar ? item.avatar : (item.title ? item.title.charAt(0) : ''),
            verified: item.verified ?? true,
            preferred: item.preferred ?? true,
          }
        })
        this.setData({ ...response, recommendations, loading: false })
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
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
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
