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
    loading: true,
    error: '',
    isSubmitting: false,
    songs: [] as Song[],
    selectedSongId: '',
    targets: [] as Target[],
    selectedTargets: [] as string[],
    budgets: [] as Budget[],
    selectedBudgetId: '',
    canSubmit: false,
    selectionSummary: '完成选择后即可开始匹配',
  },
  lifetimes: {
    async attached() {
      await this.loadOptions()
    },
  },
  methods: {
    async loadOptions() {
      this.setData({ loading: true, error: '' })
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
          loading: false,
        })
        this.updateSelectionState()
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '匹配选项加载失败',
        })
      }
    },
    chooseSong(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedSongId: event.currentTarget.dataset.id as string })
      this.updateSelectionState()
    },
    toggleTarget(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      if (this.data.selectedTargets.length === 1 && this.data.selectedTargets[0] === key) {
        wx.showToast({ title: '至少保留一个推广对象', icon: 'none' })
        return
      }
      const selectedTargets = this.data.selectedTargets.includes(key)
        ? this.data.selectedTargets.filter((target) => target !== key)
        : [...this.data.selectedTargets, key]
      const targets = this.data.targets.map((target) => ({
        ...target,
        selected: selectedTargets.includes(target.key),
      }))
      this.setData({ selectedTargets, targets })
      this.updateSelectionState()
    },
    chooseBudget(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedBudgetId: event.currentTarget.dataset.id as string })
      this.updateSelectionState()
    },
    updateSelectionState() {
      const song = this.data.songs.find((item) => item.id === this.data.selectedSongId)
      const budget = this.data.budgets.find((item) => item.id === this.data.selectedBudgetId)
      const canSubmit = Boolean(song && budget && this.data.selectedTargets.length)
      const selectionSummary = canSubmit
        ? `${song?.name} · ${this.data.selectedTargets.length} 类推广对象 · ${budget?.label}`
        : '完成选择后即可开始匹配'
      this.setData({ canSubmit, selectionSummary })
    },
    async startMatching() {
      if (!this.data.canSubmit || this.data.isSubmitting) {
        wx.showToast({ title: '请完整选择歌曲、对象与预算', icon: 'none' })
        return
      }
      this.setData({ isSubmitting: true })
      try {
        await apiRequest('/api/match', 'POST', {
          songId: this.data.selectedSongId,
          targetKeys: this.data.selectedTargets,
          budgetId: this.data.selectedBudgetId,
        })
        wx.redirectTo({ url: '/pages/ai/ai' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '智能匹配失败', icon: 'none' })
      } finally {
        this.setData({ isSubmitting: false })
      }
    },
  },
})
