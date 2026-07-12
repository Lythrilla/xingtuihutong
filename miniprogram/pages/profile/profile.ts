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
        this.setData({
          ...profile,
          isProvider: profile.user.role === 'provider',
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
    async switchRole() {
      if (this.data.actionLoading) return
      const isProvider = !this.data.isProvider
      const role = isProvider ? 'provider' : 'client'
      const roleName = isProvider ? '服务方' : '被服务方'
      const confirmation = await wx.showModal({
        title: `切换为${roleName}`,
        content: '首页数据与推荐会按新身份重新展示，已有会话和资料不会丢失。',
        confirmText: '确认切换',
      })
      if (!confirmation.confirm) return
      this.setData({ actionLoading: true })
      try {
        await apiRequest('/api/me/role', 'PUT', { role })
        app.globalData.role = role
        wx.setStorageSync('starconnect-role', role)
        await this.loadProfile()
        wx.showToast({ title: `已切换为${roleName}`, icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '角色切换失败', icon: 'none' })
      } finally {
        this.setData({ actionLoading: false })
      }
    },
    editProfile() {
      this.setData({
        sheetMode: 'edit',
        organizationInput: this.data.user.organization,
        descriptionInput: this.data.user.description,
        tagsInput: this.data.tags.join('，'),
      })
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
