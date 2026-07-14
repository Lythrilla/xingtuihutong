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
    workspaceActions: [] as Array<{
      key: string
      title: string
      description: string
      icon: string
    }>,
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
          workspaceActions:
            response.role === 'client'
              ? [
                  { key: 'ai', title: 'AI Agent', description: '分析作品与推广方向', icon: 'spark' },
                  { key: 'match', title: '发布需求', description: '拆分多个推广任务', icon: 'spark' },
                  { key: 'plaza', title: '找推广方', description: '查看能力和认证', icon: 'target' },
                ]
              : [
                  { key: 'plaza', title: '发现项目', description: '查看真实创作者', icon: 'target' },
                  { key: 'ai', title: 'AI 工作台', description: '分析合作重点', icon: 'spark' },
                  { key: 'membership', title: '联系权益', description: '会员与按次解锁', icon: 'wallet' },
                ],
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
      wx.redirectTo({ url: '/pages/ai/ai' })
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
    openDemands() {
      wx.redirectTo({ url: '/pages/demands/demands' })
    },
    openWorkspaceAction(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      const routes: Record<string, string> = {
        match: '/pages/match/match',
        plaza: '/pages/plaza/plaza',
        ai: '/pages/ai/ai',
        membership: '/pages/membership/membership',
        demands: '/pages/demands/demands',
      }
      const url = routes[key]
      if (url) wx.redirectTo({ url })
    },
    openPartner(event: WechatMiniprogram.TouchEvent) {
      if (!this.data.isApproved) {
        this.openOnboarding()
        return
      }
      const partnerId = event.currentTarget.dataset.id as string
      if (!partnerId) return
      wx.navigateTo({
        url: `/pages/partner-detail/partner-detail?id=${encodeURIComponent(partnerId)}`,
      })
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
