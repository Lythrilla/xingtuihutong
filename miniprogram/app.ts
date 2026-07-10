App<IAppOption>({
  globalData: {
    role: 'provider',
  },
  onLaunch() {
    const role = wx.getStorageSync('starconnect-role')
    if (role === 'provider' || role === 'client') {
      this.globalData.role = role
    }
  },
})