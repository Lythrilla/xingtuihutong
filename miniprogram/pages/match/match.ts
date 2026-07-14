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
  availableProviders: number
}

interface DemandDraft {
  songId: string
  targetKeys: string[]
  budgetId: string
  goal: string
  cycle: string
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
    selectedTasks: [] as Target[],
    budgets: [] as Budget[],
    selectedBudgetId: '',
    availableProviders: 0,
    goal: '',
    cycles: ['7 天', '14 天', '30 天', '60 天'],
    selectedCycle: '14 天',
    canSubmit: false,
    activeStep: 1,
    selectionSummary: '先选择作品并填写推广目标',
    savedHint: '',
  },
  lifetimes: {
    async attached() {
      await this.loadOptions()
    },
  },
  methods: {
    retry() {
      return this.loadOptions()
    },
    async loadOptions() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<MatchBootstrap>('/api/match/bootstrap')
        const draft = getDemandDraft()
        const validTargetKeys = response.targets.map((target) => target.key)
        const selectedTargets = (draft?.targetKeys || []).filter((key) =>
          validTargetKeys.includes(key),
        )
        const targetKeys = selectedTargets.length
          ? selectedTargets
          : response.targets.length
            ? [response.targets[0].key]
            : []
        const songId = response.songs.some((song) => song.id === draft?.songId)
          ? draft?.songId || ''
          : response.songs[0]?.id || ''
        const budgetId = response.budgets.some((budget) => budget.id === draft?.budgetId)
          ? draft?.budgetId || ''
          : response.budgets[1]?.id || response.budgets[0]?.id || ''
        const targets = response.targets.map((target) => ({
          ...target,
          selected: targetKeys.includes(target.key),
        }))
        this.setData({
          songs: response.songs,
          selectedSongId: songId,
          targets,
          selectedTargets: targetKeys,
          selectedTasks: targets.filter((target) => target.selected),
          budgets: response.budgets,
          availableProviders: response.availableProviders,
          selectedBudgetId: budgetId,
          goal: draft?.goal || '',
          selectedCycle: draft?.cycle || '14 天',
          savedHint: draft ? '已恢复上次未发布的草稿' : '',
          loading: false,
        })
        this.updateSelectionState()
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '需求选项加载失败',
        })
      }
    },
    chooseSong(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedSongId: event.currentTarget.dataset.id as string })
      this.afterDraftChange()
    },
    toggleTarget(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      if (this.data.selectedTargets.length === 1 && this.data.selectedTargets[0] === key) {
        wx.showToast({ title: '至少保留一个推广任务', icon: 'none' })
        return
      }
      const selectedTargets = this.data.selectedTargets.includes(key)
        ? this.data.selectedTargets.filter((target) => target !== key)
        : [...this.data.selectedTargets, key]
      const targets = this.data.targets.map((target) => ({
        ...target,
        selected: selectedTargets.includes(target.key),
      }))
      this.setData({
        selectedTargets,
        targets,
        selectedTasks: targets.filter((target) => target.selected),
      })
      this.afterDraftChange()
    },
    chooseBudget(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedBudgetId: event.currentTarget.dataset.id as string })
      this.afterDraftChange()
    },
    updateGoal(event: WechatMiniprogram.Input) {
      this.setData({ goal: event.detail.value })
      this.afterDraftChange()
    },
    chooseCycle(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedCycle: event.currentTarget.dataset.value as string })
      this.afterDraftChange()
    },
    afterDraftChange() {
      this.updateSelectionState()
      saveDemandDraft({
        songId: this.data.selectedSongId,
        targetKeys: this.data.selectedTargets,
        budgetId: this.data.selectedBudgetId,
        goal: this.data.goal,
        cycle: this.data.selectedCycle,
      })
      this.setData({ savedHint: '草稿已自动保存' })
    },
    updateSelectionState() {
      const song = this.data.songs.find((item) => item.id === this.data.selectedSongId)
      const budget = this.data.budgets.find((item) => item.id === this.data.selectedBudgetId)
      const hasTasks = this.data.selectedTargets.length > 0
      const hasGoal = this.data.goal.trim().length >= 8
      const canSubmit = Boolean(song && budget && hasTasks && hasGoal)
      const activeStep = !song ? 1 : !hasGoal ? 2 : !hasTasks || !budget ? 3 : 4
      const selectionSummary = canSubmit
        ? `${song?.name} · ${this.data.selectedTargets.length} 个推广任务 · ${budget?.label} · ${this.data.selectedCycle}`
        : !hasGoal
          ? '推广目标至少填写 8 个字，方便服务方准确判断'
          : '请完整选择推广任务与预算'
      this.setData({ canSubmit, activeStep, selectionSummary })
    },
    openAIAssistant() {
      const song = this.data.songs.find((item) => item.id === this.data.selectedSongId)
      const taskNames = this.data.selectedTasks.map((task) => task.title).join('、')
      if (!song || !taskNames) {
        wx.showToast({ title: '先选择作品和推广任务', icon: 'none' })
        return
      }
      const prompt = `请帮我完善《${song.name}》的推广需求草稿。当前目标：${this.data.goal || '尚未填写'}；计划任务：${taskNames}；周期：${this.data.selectedCycle}。请只生成需求描述和交付要点，不生成报价。`
      wx.setStorageSync('starconnect-ai-prefill', prompt)
      wx.redirectTo({ url: '/pages/ai/ai' })
    },
    startMatching() {
      if (!this.data.canSubmit || this.data.isSubmitting) {
        wx.showToast({ title: this.data.selectionSummary, icon: 'none' })
        return
      }
      const song = this.data.songs.find((item) => item.id === this.data.selectedSongId)
      const budget = this.data.budgets.find((item) => item.id === this.data.selectedBudgetId)
      wx.showModal({
        title: '确认发布推广需求',
        content: `${song?.name || '当前作品'}将发布 ${this.data.selectedTargets.length} 个推广任务，整体预算 ${budget?.label || ''}，周期 ${this.data.selectedCycle}。发布后可继续查看推荐服务方。`,
        confirmText: '确认发布',
        success: (result) => {
          if (result.confirm) void this.submitMatch()
        },
      })
    },
    async submitMatch() {
      this.setData({ isSubmitting: true })
      try {
        const response = await apiRequest<{ matchId: string }>('/api/match', 'POST', {
          songId: this.data.selectedSongId,
          targetKeys: this.data.selectedTargets,
          budgetId: this.data.selectedBudgetId,
          goal: this.data.goal.trim(),
          cycle: this.data.selectedCycle,
        })
        wx.removeStorageSync('starconnect-demand-draft')
        wx.setStorageSync('starconnect-latest-demand', {
          id: response.matchId,
          goal: this.data.goal.trim(),
          cycle: this.data.selectedCycle,
          taskCount: this.data.selectedTargets.length,
        })
        wx.showToast({ title: '需求已发布', icon: 'success' })
        setTimeout(() => wx.redirectTo({ url: '/pages/demands/demands' }), 500)
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '需求发布失败', icon: 'none' })
      } finally {
        this.setData({ isSubmitting: false })
      }
    },
  },
})

function getDemandDraft(): DemandDraft | null {
  const value = wx.getStorageSync('starconnect-demand-draft') as unknown
  if (!value || typeof value !== 'object') return null
  const draft = value as Partial<DemandDraft>
  if (!Array.isArray(draft.targetKeys)) return null
  return {
    songId: typeof draft.songId === 'string' ? draft.songId : '',
    targetKeys: draft.targetKeys.filter((key): key is string => typeof key === 'string'),
    budgetId: typeof draft.budgetId === 'string' ? draft.budgetId : '',
    goal: typeof draft.goal === 'string' ? draft.goal : '',
    cycle: typeof draft.cycle === 'string' ? draft.cycle : '14 天',
  }
}

function saveDemandDraft(draft: DemandDraft) {
  wx.setStorageSync('starconnect-demand-draft', draft)
}
