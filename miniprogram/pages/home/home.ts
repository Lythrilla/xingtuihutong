import { apiRequest } from '../../utils/api'

export {}

const app = getApp<IAppOption>()

interface Recommendation {
  id: string
  avatar: string
  avatarClass: string
  avatarIsImage: boolean
  verified: boolean
  preferred: boolean
  title: string
  subtitle: string
  score: string
  price: string
}

interface HomeResponse {
  headerSubtitle: string
  name: string
  role: 'provider' | 'client'
  onboardingStatus: 'draft' | 'pending' | 'approved' | 'rejected'
  statusTitle: string
  statusDescription: string
  metrics: Array<{ value: string; label: string }>
  recommendations: Recommendation[]
}

Component({
  data: {
    loading: true,
    error: '',
    greeting: '早上好',
    role: 'client' as 'provider' | 'client',
    isCreator: true,
    isApproved: false,
    onboardingStatus: 'draft',
    statusTitle: '',
    statusDescription: '',
    headerSubtitle: '',
    name: '',
    metrics: [] as HomeResponse['metrics'],
    recommendations: [] as Recommendation[],
    connectingId: '',
  },
  lifetimes: {
    attached() {
      this.setData({ greeting: greetingForHour(new Date().getHours()) })
      this.loadHome()
    },
  },
  methods: {
    retry() {
      return this.loadHome()
    },
    async loadHome() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<HomeResponse>('/api/home')
        app.globalData.role = response.role
        app.globalData.onboardingStatus = response.onboardingStatus
        wx.setStorageSync('starconnect-onboarding-status', response.onboardingStatus)
        const recommendations = (response.recommendations || []).map((item) => {
          const hasImageAvatar = !!item.avatar && (item.avatar.indexOf('http') === 0 || item.avatar.indexOf('/') === 0)
          return {
            ...item,
            avatar: hasImageAvatar ? item.avatar : (item.title ? item.title.charAt(0) : ''),
            avatarIsImage: hasImageAvatar,
            verified: item.verified ?? true,
            preferred: item.preferred ?? true,
          }
        })
        this.setData({
          ...response,
          isCreator: response.role === 'client',
          isApproved: response.onboardingStatus === 'approved',
          recommendations,
          loading: false,
        })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '首页加载失败',
        })
      }
    },
    openPrimary() {
      wx.redirectTo({
        url: this.data.isCreator ? '/pages/match/match' : '/pages/plaza/plaza',
      })
    },
    openOnboarding() {
      wx.redirectTo({ url: '/pages/onboarding/onboarding' })
    },
    openStatus() {
      if (!this.data.isApproved) this.openOnboarding()
    },
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
    async contactPartner(event: WechatMiniprogram.TouchEvent) {
      if (!this.data.isApproved) {
        this.openOnboarding()
        return
      }
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
