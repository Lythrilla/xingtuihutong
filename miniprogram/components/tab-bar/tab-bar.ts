export {}

Component({
  properties: {
    active: {
      type: String,
      value: 'home',
    },
  },
  data: {
    items: [
      { key: 'home', label: '首页', icon: '⌂', url: '/pages/home/home' },
      { key: 'plaza', label: '广场', icon: '◇', url: '/pages/plaza/plaza' },
      { key: 'match', label: '匹配', icon: '+', url: '/pages/match/match' },
      { key: 'ai', label: 'AI推荐', icon: '✦', url: '/pages/ai/ai' },
      { key: 'profile', label: '我的', icon: '○', url: '/pages/profile/profile' },
    ],
  },
  methods: {
    navigate(event: WechatMiniprogram.TouchEvent) {
      const item = event.currentTarget.dataset.item as { key: string; url: string }
      if (item.key !== this.data.active) {
        wx.redirectTo({ url: item.url })
      }
    },
  },
})
