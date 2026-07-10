import { ensureSession } from './utils/api'

App<IAppOption>({
  globalData: {
    role: 'provider',
    apiReady: false,
  },
  onLaunch() {
    const role = wx.getStorageSync('starconnect-role')
    if (role === 'provider' || role === 'client') {
      this.globalData.role = role
    }
    void ensureSession(this.globalData.role)
      .then((session) => {
        this.globalData.role = session.user.role
        this.globalData.apiReady = true
      })
      .catch(() => {
        this.globalData.apiReady = false
      })
  },
})