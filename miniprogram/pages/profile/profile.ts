import { apiRequest, SessionUser } from '../../utils/api'

export {}

const app = getApp<IAppOption>()

interface ProfileResponse {
  user: SessionUser
  roleLabel: string
  stats: Array<{ value: string; label: string }>
  certifications: Array<{
    id: string
    iconClass: string
    colorClass: string
    title: string
    status: string
  }>
  tags: string[]
  cases: Array<{
    id: string
    colorClass: string
    caseType: string
    name: string
    resultText: string
  }>
  walletBalance: number
}

Component({
  data: {
    loading: true,
    error: '',
    actionLoading: false,
    sheetMode: '',
    isProvider: true,
    isApproved: false,
    user: {} as SessionUser,
    roleLabel: '',
    stats: [] as ProfileResponse['stats'],
    certifications: [] as ProfileResponse['certifications'],
    tags: [] as string[],
    cases: [] as ProfileResponse['cases'],
    walletBalance: '',
    walletBalanceCents: 0,
    organizationInput: '',
    descriptionInput: '',
    tagsInput: '',
    withdrawInput: '',
    quickActions: [] as Array<{ key: string; label: string; icon: string }>,
    serviceActions: [] as Array<{
      key: string
      label: string
      description: string
      icon: string
    }>,
  },
  lifetimes: {
    async attached() {
      await this.loadProfile()
    },
  },
  methods: {
    retry() {
      return this.loadProfile()
    },
    async loadProfile() {
      this.setData({ loading: true, error: '' })
      try {
        const profile = await apiRequest<ProfileResponse>('/api/profile')
        app.globalData.role = profile.user.role
        app.globalData.onboardingStatus = profile.user.onboardingStatus
        wx.setStorageSync('starconnect-onboarding-status', profile.user.onboardingStatus)
        const isProvider = profile.user.role === 'provider'
        const isApproved = profile.user.onboardingStatus === 'approved'
        this.setData({
          ...profile,
          isProvider,
          isApproved,
          quickActions: !isApproved
            ? [
                { key: 'onboarding', label: '完成入驻', icon: 'shield' },
                { key: 'plaza', label: '浏览广场', icon: 'target' },
                { key: 'messages', label: '消息', icon: 'audio' },
                { key: 'analytics', label: '数据', icon: 'target' },
              ]
            : isProvider
            ? [
                { key: 'plaza', label: '找创作者', icon: 'target' },
                { key: 'agent', label: 'AI Agent', icon: 'spark' },
                { key: 'analytics', label: '服务数据', icon: 'target' },
                { key: 'messages', label: '合作会话', icon: 'audio' },
              ]
            : [
                { key: 'match', label: '发推广', icon: 'spark' },
                { key: 'plaza', label: '找推广方', icon: 'target' },
                { key: 'analytics', label: '推广数据', icon: 'target' },
                { key: 'messages', label: '合作会话', icon: 'audio' },
              ],
          serviceActions: !isApproved
            ? [
                {
                  key: 'onboarding',
                  label: isProvider ? '完成推广方入驻' : '完成创作者入驻',
                  description: '提交真实资料并等待平台审核',
                  icon: 'shield',
                },
              ]
            : isProvider
            ? [
                { key: 'onboarding', label: '服务方入驻资料', description: '主体、能力与审核状态', icon: 'shield' },
                { key: 'ai', label: 'AI Agent 工作台', description: '分析创作者项目与合作重点', icon: 'spark' },
                { key: 'favorites', label: '收藏的创作者', description: '回看感兴趣的创作者项目', icon: 'target' },
              ]
            : [
                { key: 'onboarding', label: '创作者入驻资料', description: '身份、作品与审核状态', icon: 'shield' },
                { key: 'match', label: '发布推广需求', description: '选择作品、方向与合作预算', icon: 'spark' },
                { key: 'favorites', label: '收藏的推广方', description: '回看感兴趣的推广服务', icon: 'target' },
              ],
          walletBalance: formatMoney(profile.walletBalance),
          walletBalanceCents: profile.walletBalance,
          loading: false,
        })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '资料加载失败',
        })
      }
    },
    editProfile() {
      wx.redirectTo({ url: '/pages/onboarding/onboarding' })
    },
    updateOrganization(event: WechatMiniprogram.Input) {
      this.setData({ organizationInput: event.detail.value })
    },
    updateDescription(event: WechatMiniprogram.Input) {
      this.setData({ descriptionInput: event.detail.value })
    },
    updateTags(event: WechatMiniprogram.Input) {
      this.setData({ tagsInput: event.detail.value })
    },
    updateWithdraw(event: WechatMiniprogram.Input) {
      this.setData({ withdrawInput: event.detail.value })
    },
    closeSheet() {
      if (this.data.actionLoading) return
      this.setData({ sheetMode: '', withdrawInput: '' })
    },
    preventClose() {},
    openQuickAction(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      if (key === 'onboarding') {
        wx.redirectTo({ url: '/pages/onboarding/onboarding' })
        return
      }
      const routes: Record<string, string> = {
        agent: '/pages/ai/ai',
        analytics: '/pages/analytics/analytics',
        plaza: '/pages/plaza/plaza',
        match: '/pages/match/match',
        messages: '/pages/messages/messages',
      }
      const url = routes[key]
      if (url) wx.redirectTo({ url })
    },
    openServiceAction(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      if (key === 'onboarding') {
        wx.redirectTo({ url: '/pages/onboarding/onboarding' })
        return
      }
      const routes: Record<string, string> = {
        ai: '/pages/ai/ai',
        analytics: '/pages/analytics/analytics',
        favorites: '/pages/plaza/plaza',
        match: '/pages/match/match',
      }
      const url = routes[key]
      if (url) wx.redirectTo({ url })
    },
    async saveProfile() {
      const organization = this.data.organizationInput.trim()
      const description = this.data.descriptionInput.trim()
      if (!organization) {
        wx.showToast({ title: '请输入机构或个人名称', icon: 'none' })
        return
      }
      if (!description) {
        wx.showToast({ title: '请填写业务简介', icon: 'none' })
        return
      }
      const tags = this.data.tagsInput
        .split(/[,，]/)
        .map((tag) => tag.trim())
        .filter(Boolean)
        .slice(0, 8)
      this.setData({ actionLoading: true })
      try {
        await apiRequest('/api/profile', 'PUT', { organization, description, tags })
        this.setData({ sheetMode: '' })
        await this.loadProfile()
        wx.showToast({ title: '资料已更新', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '资料更新失败', icon: 'none' })
      } finally {
        this.setData({ actionLoading: false })
      }
    },
    withdraw() {
      this.setData({ sheetMode: 'withdraw', withdrawInput: '' })
    },
    fillAllBalance() {
      this.setData({ withdrawInput: (this.data.walletBalanceCents / 100).toFixed(2) })
    },
    async submitWithdrawal() {
      const amount = Math.round(Number(this.data.withdrawInput) * 100)
      if (!Number.isFinite(amount) || amount <= 0) {
        wx.showToast({ title: '请输入有效金额', icon: 'none' })
        return
      }
      if (amount > this.data.walletBalanceCents) {
        wx.showToast({ title: '提现金额不能超过可用余额', icon: 'none' })
        return
      }
      this.setData({ actionLoading: true })
      try {
        await apiRequest('/api/wallet/withdraw', 'POST', { amount })
        this.setData({ sheetMode: '', withdrawInput: '' })
        await this.loadProfile()
        wx.showToast({ title: '提现申请已提交', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '提现申请失败', icon: 'none' })
      } finally {
        this.setData({ actionLoading: false })
      }
    },
  },
})

function formatMoney(cents: number): string {
  return `¥${(cents / 100).toFixed(2)}`
}
