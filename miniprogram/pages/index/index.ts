import { ensureSession } from '../../utils/api'

export {}

const app = getApp<IAppOption>()

Component({
  data: {
    selectedRole: '',
    agreed: false,
    entering: false,
    statusBarHeight: 20,
    navigationHeight: 44,
  },
  lifetimes: {
    attached() {
      const menu = wx.getMenuButtonBoundingClientRect()
      const systemInfo = wx.getSystemInfoSync()
      const statusBarHeight = systemInfo.statusBarHeight || 20
      const gap = Math.max(menu.top - statusBarHeight, 4)
      const menuHeight = menu.height || 32
      this.setData({
        statusBarHeight,
        navigationHeight: Math.max(menuHeight + gap * 2, 52),
      })
    },
  },
  methods: {
    goAgreement() {
      wx.navigateTo({ url: '/pages/agreement/agreement' })
    },
    goPrivacy() {
      wx.navigateTo({ url: '/pages/privacy/privacy' })
    },
    chooseRole(event: WechatMiniprogram.TouchEvent) {
      const role = event.currentTarget.dataset.role as 'provider' | 'client'
      this.setData({ selectedRole: role })
    },
    toggleAgree(event: WechatMiniprogram.CheckboxGroupChange) {
      this.setData({ agreed: event.detail.value.includes('agreed') })
    },
    async enterApp() {
      if (this.data.entering) return
      const role = this.data.selectedRole as 'provider' | 'client'
      if (!role) {
        wx.showToast({ title: '请先选择使用身份', icon: 'none' })
        return
      }
      if (!this.data.agreed) {
        wx.showToast({ title: '请先阅读并同意用户协议与隐私政策', icon: 'none' })
        return
      }
      this.setData({ entering: true })
      try {
        // 清除旧 token，确保以选定角色创建全新会话
        wx.removeStorageSync('starconnect-token')
        const session = await ensureSession(role)
        const user = session.user
        app.globalData.role = user.role
        app.globalData.onboardingStatus = user.onboardingStatus
        app.globalData.apiReady = true
        wx.setStorageSync('starconnect-role', user.role)
        wx.setStorageSync('starconnect-onboarding-status', user.onboardingStatus)
        wx.redirectTo({
          url: user.onboardingStatus === 'approved' || user.onboardingStatus === 'pending'
            ? '/pages/home/home'
            : '/pages/onboarding/onboarding',
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '服务连接失败', icon: 'none' })
      } finally {
        this.setData({ entering: false })
      }
    },
  },
})
