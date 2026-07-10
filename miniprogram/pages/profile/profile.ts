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
    isProvider: true,
    user: {} as SessionUser,
    roleLabel: '',
    stats: [] as ProfileResponse['stats'],
    certifications: [] as ProfileResponse['certifications'],
    tags: [] as string[],
    cases: [] as ProfileResponse['cases'],
    walletBalance: '',
  },
  lifetimes: {
    async attached() {
      await this.loadProfile()
    },
  },
  methods: {
    async loadProfile() {
      try {
        const profile = await apiRequest<ProfileResponse>('/api/profile')
        app.globalData.role = profile.user.role
        this.setData({
          ...profile,
          isProvider: profile.user.role === 'provider',
          walletBalance: formatMoney(profile.walletBalance),
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '资料加载失败', icon: 'none' })
      }
    },
    async switchRole() {
      const isProvider = !this.data.isProvider
      const role = isProvider ? 'provider' : 'client'
      try {
        await apiRequest('/api/me/role', 'PUT', { role })
        app.globalData.role = role
        wx.setStorageSync('starconnect-role', role)
        await this.loadProfile()
        wx.showToast({ title: `已切换为${isProvider ? '服务方' : '被服务方'}`, icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '角色切换失败', icon: 'none' })
      }
    },
    async editProfile() {
      const result = await wx.showModal({
        title: '编辑简介',
        content: this.data.user.description,
        editable: true,
        placeholderText: '介绍您的业务与合作方向',
      })
      if (!result.confirm || !result.content?.trim()) return
      try {
        await apiRequest('/api/profile', 'PUT', { description: result.content.trim() })
        await this.loadProfile()
        wx.showToast({ title: '资料已更新', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '资料更新失败', icon: 'none' })
      }
    },
    async withdraw() {
      const result = await wx.showModal({
        title: '申请提现',
        editable: true,
        placeholderText: '输入提现金额（元）',
      })
      if (!result.confirm) return
      const amount = Math.round(Number(result.content) * 100)
      if (!Number.isFinite(amount) || amount <= 0) {
        wx.showToast({ title: '请输入有效金额', icon: 'none' })
        return
      }
      try {
        await apiRequest('/api/wallet/withdraw', 'POST', { amount })
        await this.loadProfile()
        wx.showToast({ title: '提现申请已提交', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '提现申请失败', icon: 'none' })
      }
    },
  },
})

function formatMoney(cents: number): string {
  return `¥${(cents / 100).toFixed(2)}`
}
