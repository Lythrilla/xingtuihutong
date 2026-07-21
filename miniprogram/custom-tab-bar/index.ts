export {}

const app = getApp<IAppOption>()

const ONBOARDING_URL = '/pages/onboarding/onboarding'

interface TabItem {
  key: string
  label: string
  iconClass: string
  url: string
}

Component({
  data: {
    active: 'home',
    navigating: false,
    items: [] as TabItem[],
  },
  lifetimes: {
    attached() {
      this.updateState()
    },
  },
  methods: {
    setActive(active: string) {
      this.setData({ active, navigating: false })
      this.updateState()
    },
    updateState() {
      const isCreator = app.globalData.role === 'client'
      const isApproved = app.globalData.onboardingStatus === 'approved'
      this.setData({
        items: [
          { key: 'home', label: '首页', iconClass: 'home', url: '/pages/home/home' },
          {
            key: 'plaza',
            label: isCreator ? '找推广' : '找创作者',
            iconClass: 'plaza',
            url: '/pages/plaza/plaza',
          },
          {
            key: 'match',
            label: isApproved ? 'AI' : '入驻',
            iconClass: 'match',
            url: isApproved ? '/pages/ai/ai' : ONBOARDING_URL,
          },
          { key: 'messages', label: '消息', iconClass: 'messages', url: '/pages/messages/messages' },
          { key: 'profile', label: '我的', iconClass: 'profile', url: '/pages/profile/profile' },
        ],
      })
    },
    navigate(event: WechatMiniprogram.TouchEvent) {
      const item = event.currentTarget.dataset.item as TabItem
      if (item.key === this.data.active || this.data.navigating) return
      if (item.url === ONBOARDING_URL) {
        wx.navigateTo({ url: item.url })
        return
      }
      this.setData({ navigating: true })
      wx.switchTab({
        url: item.url,
        complete: () => this.setData({ navigating: false }),
      })
    },
  },
})
