export {}

const app = getApp<IAppOption>()

Component({
  data: {
    selectedRole: 'provider',
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
    selectRole(event: WechatMiniprogram.TouchEvent) {
      const role = event.currentTarget.dataset.role as 'provider' | 'client'
      this.setData({ selectedRole: role })
    },
    enterApp() {
      const role = this.data.selectedRole as 'provider' | 'client'
      app.globalData.role = role
      wx.setStorageSync('starconnect-role', role)
      wx.redirectTo({ url: '/pages/home/home' })
    },
  },
})
