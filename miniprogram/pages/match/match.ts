import { apiRequest } from '../../utils/api'

export {}

interface Song {
  id: string
  name: string
  artist: string
  coverClass: string
}

interface Target {
  key: string
  iconClass: string
  title: string
  description: string
  selected: boolean
}

interface Budget {
  id: string
  label: string
}

interface MatchBootstrap {
  songs: Song[]
  targets: Omit<Target, 'selected'>[]
  budgets: Budget[]
}

Component({
  data: {
    songs: [] as Song[],
    selectedSongId: '',
    targets: [] as Target[],
    selectedTargets: [] as string[],
    budgets: [] as Budget[],
    selectedBudgetId: '',
  },
  lifetimes: {
    async attached() {
      try {
        const response = await apiRequest<MatchBootstrap>('/api/match/bootstrap')
        const selectedTargets = response.targets.length ? [response.targets[0].key] : []
        this.setData({
          songs: response.songs,
          selectedSongId: response.songs[0]?.id || '',
          targets: response.targets.map((target) => ({
            ...target,
            selected: selectedTargets.includes(target.key),
          })),
          selectedTargets,
          budgets: response.budgets,
          selectedBudgetId: response.budgets[1]?.id || response.budgets[0]?.id || '',
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '匹配选项加载失败', icon: 'none' })
      }
    },
  },
  methods: {
    chooseSong(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedSongId: event.currentTarget.dataset.id as string })
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
      this.setData({ selectedBudgetId: event.currentTarget.dataset.id as string })
    },
    async startMatching() {
      if (!this.data.selectedSongId || !this.data.selectedBudgetId || !this.data.selectedTargets.length) {
        wx.showToast({ title: '请完整选择歌曲、对象与预算', icon: 'none' })
        return
      }
      wx.showLoading({ title: 'AI 正在匹配' })
      try {
        await apiRequest('/api/match', 'POST', {
          songId: this.data.selectedSongId,
          targetKeys: this.data.selectedTargets,
          budgetId: this.data.selectedBudgetId,
        })
        wx.hideLoading()
        wx.redirectTo({ url: '/pages/ai/ai' })
      } catch (error) {
        wx.hideLoading()
        wx.showToast({ title: error instanceof Error ? error.message : '智能匹配失败', icon: 'none' })
      }
    },
  },
})
