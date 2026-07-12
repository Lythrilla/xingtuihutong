export {}

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
      { key: 'match', label: 'AI', iconClass: 'match', url: '/pages/match/match' },
      { key: 'messages', label: '消息', iconClass: 'messages', url: '/pages/messages/messages' },
      { key: 'profile', label: '我的', iconClass: 'profile', url: '/pages/profile/profile' },
    ],
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
