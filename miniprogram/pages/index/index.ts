export {}

const app = getApp<IAppOption>()

Component({
  data: {
    selectedRole: 'provider',
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
