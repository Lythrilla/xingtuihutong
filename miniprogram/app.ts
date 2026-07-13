import { ensureSession } from './utils/api'

App<IAppOption>({
  globalData: {
    role: 'provider',
    onboardingStatus: 'draft',
    apiReady: false,
  },
  onLaunch() {
    const role = wx.getStorageSync('starconnect-role')
    if (role === 'provider' || role === 'client') {
      this.globalData.role = role
    }
    const onboardingStatus = wx.getStorageSync('starconnect-onboarding-status')
    if (['draft', 'pending', 'approved', 'rejected'].includes(onboardingStatus)) {
      this.globalData.onboardingStatus = onboardingStatus
    }
    void ensureSession(this.globalData.role)
      .then((session) => {
        this.globalData.role = session.user.role
        this.globalData.onboardingStatus = session.user.onboardingStatus
        this.globalData.apiReady = true
        wx.setStorageSync('starconnect-onboarding-status', session.user.onboardingStatus)
      })
      .catch(() => {
        this.globalData.apiReady = false
      })
  },
})