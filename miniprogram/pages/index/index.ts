import { apiRequest, ensureSession, SessionUser } from '../../utils/api'

export {}

const app = getApp<IAppOption>()

Component({
  data: {
    selectedRole: 'provider',
    agreed: false,
    entering: false,
    statusBarHeight: 20,
    navigationHeight: 44,
    rightInset: 96,
  },
  lifetimes: {
    attached() {
      const menu = wx.getMenuButtonBoundingClientRect()
      const systemInfo = wx.getSystemInfoSync()
      const statusBarHeight = systemInfo.statusBarHeight || 20
      const gap = Math.max(menu.top - statusBarHeight, 4)
      const menuHeight = menu.height || 32
      const rightInset =
        menu.left > systemInfo.windowWidth / 2 ? systemInfo.windowWidth - menu.left + 10 : 12
      this.setData({
        statusBarHeight,
        navigationHeight: Math.max(menuHeight + gap * 2, 52),
        rightInset,
      })
    },
  },
  methods: {
    chooseRole(event: WechatMiniprogram.TouchEvent) {
      const role = event.currentTarget.dataset.role as 'provider' | 'client'
      this.setData({ selectedRole: role })
      if (!this.data.agreed) {
        wx.showToast({ title: '请先阅读并同意用户协议与隐私政策', icon: 'none' })
        return
      }
      this.enterApp()
    },
    toggleAgree() {
      this.setData({ agreed: !this.data.agreed })
    },
    async enterApp() {
      if (this.data.entering) return
      const role = this.data.selectedRole as 'provider' | 'client'
      this.setData({ entering: true })
      try {
        await ensureSession(role)
        const user = await apiRequest<SessionUser>('/api/me/role', 'PUT', { role })
        app.globalData.role = user.role
        app.globalData.apiReady = true
        wx.setStorageSync('starconnect-role', user.role)
        wx.redirectTo({ url: '/pages/home/home' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '服务连接失败', icon: 'none' })
      } finally {
        this.setData({ entering: false })
      }
    },
  },
})
