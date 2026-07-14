export {}

const app = getApp<IAppOption>()

Component({
  properties: {
    active: {
      type: String,
      value: 'home',
    },
  },
  data: {
    navigating: false,
    items: [
      { key: 'home', label: '首页', iconClass: 'home', url: '/pages/home/home' },
      { key: 'plaza', label: '广场', iconClass: 'plaza', url: '/pages/plaza/plaza' },
      { key: 'match', label: 'AI', iconClass: 'match', url: '/pages/ai/ai' },
      { key: 'messages', label: '消息', iconClass: 'messages', url: '/pages/messages/messages' },
      { key: 'profile', label: '我的', iconClass: 'profile', url: '/pages/profile/profile' },
    ],
  },
  lifetimes: {
    attached() {
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
            url: isApproved ? '/pages/ai/ai' : '/pages/onboarding/onboarding',
          },
          { key: 'messages', label: '消息', iconClass: 'messages', url: '/pages/messages/messages' },
          { key: 'profile', label: '我的', iconClass: 'profile', url: '/pages/profile/profile' },
        ],
      })
    },
  },
  methods: {
    navigate(event: WechatMiniprogram.TouchEvent) {
      const item = event.currentTarget.dataset.item as { key: string; url: string }
      if (item.key === this.data.active || this.data.navigating) return
      this.setData({ navigating: true })
      wx.redirectTo({
        url: item.url,
        fail: () => this.setData({ navigating: false }),
      })
    },
  },
})
