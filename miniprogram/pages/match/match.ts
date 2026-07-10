export {}

Component({
  data: {
    songs: [
      { name: '微醺日落', artist: '林屿', cover: 'sunset' },
      { name: '海风来信', artist: '鹿野乐队', cover: 'ocean' },
      { name: '逆光飞行', artist: 'SONA', cover: 'violet' },
    ],
    selectedSong: 0,
    targets: [
      { key: 'creator', icon: '◉', title: '短视频创作者', desc: '内容种草与矩阵传播', selected: true },
      { key: 'campus', icon: '◇', title: '校园音乐人', desc: '年轻圈层与线下活动', selected: false },
      { key: 'brand', icon: '✦', title: '品牌营销机构', desc: '商业联名与场景曝光', selected: false },
      { key: 'media', icon: '⌁', title: '音乐媒体', desc: '榜单、乐评与媒体传播', selected: false },
    ],
    selectedTargets: ['creator'],
    budgets: ['¥5,000 以下', '¥5,000 - 20,000', '¥20,000 - 50,000', '¥50,000 以上'],
    selectedBudget: 1,
  },
  methods: {
    chooseSong(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedSong: Number(event.currentTarget.dataset.index) })
    },
    toggleTarget(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      const selectedTargets = this.data.selectedTargets.includes(key)
        ? this.data.selectedTargets.filter((target) => target !== key)
        : [...this.data.selectedTargets, key]
      const targets = this.data.targets.map((target) => ({
        ...target,
        selected: selectedTargets.includes(target.key),
      }))
      this.setData({ selectedTargets, targets })
    },
    chooseBudget(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedBudget: Number(event.currentTarget.dataset.index) })
    },
    startMatching() {
      wx.showLoading({ title: 'AI 正在匹配' })
      setTimeout(() => {
        wx.hideLoading()
        wx.redirectTo({ url: '/pages/ai/ai' })
      }, 700)
    },
  },
})
