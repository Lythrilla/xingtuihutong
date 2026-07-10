export {}

Component({
  data: {
    activeTab: 'plans',
    tabs: [
      { key: 'plans', label: '推荐方案' },
      { key: 'partners', label: '推荐伙伴' },
      { key: 'tasks', label: '推荐任务' },
    ],
    plans: [
      {
        icon: 'video',
        iconClass: 'aqua',
        title: '短视频内容矩阵方案',
        type: '增长推荐',
        desc: '用 12 位垂类创作者完成预热、爆点与长尾传播，适合《微醺日落》。',
        tags: ['预计曝光 85w+', '周期 14 天'],
        budget: '¥18,000',
        score: 98,
      },
      {
        icon: 'campus',
        iconClass: 'violet',
        title: '校园音乐人共创推广',
        type: '圈层推荐',
        desc: '联动 8 所高校音乐社团完成翻唱、路演和校园歌单收录。',
        tags: ['年轻乐迷', '线下联动'],
        budget: '¥6,500',
        score: 95,
      },
      {
        icon: 'briefcase',
        iconClass: 'blue',
        title: '品牌场景合作方案',
        type: '商业推荐',
        desc: '匹配生活方式品牌，以主题短片和门店空间音乐完成联合曝光。',
        tags: ['商业授权', '品牌联名'],
        budget: '¥28,000',
        score: 91,
      },
    ],
    refreshText: '换一批推荐',
  },
  methods: {
    changeTab(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      this.setData({ activeTab: key })
    },
    refresh() {
      this.setData({ refreshText: '已更新推荐' })
      wx.showToast({ title: 'AI 已重新分析合作池', icon: 'none' })
    },
    viewPlan(event: WechatMiniprogram.TouchEvent) {
      const title = event.currentTarget.dataset.title as string
      wx.showToast({ title: `已收藏：${title}`, icon: 'none' })
    },
  },
})
