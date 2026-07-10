import { apiRequest } from '../../utils/api'

export {}

interface Tab {
  key: string
  label: string
}

interface Plan {
  id: string
  iconClass: string
  colorClass: string
  title: string
  planType: string
  description: string
  tags: string[]
  budgetAmount: number
  budget: string
  score: number
  saved: boolean
}

interface AiResponse {
  insight: {
    title: string
    description: string
  }
  tabs: Tab[]
  plans: Omit<Plan, 'budget' | 'saved'>[]
}

Component({
  data: {
    loading: true,
    error: '',
    refreshing: false,
    savingId: '',
    activeTab: 'plans',
    tabs: [] as Tab[],
    insightTitle: '',
    insightDescription: '',
    plans: [] as Plan[],
    refreshText: '换一批推荐',
  },
  lifetimes: {
    async attached() {
      await this.loadPlans(false)
    },
  },
  methods: {
    retry() {
      return this.loadPlans(false)
    },
    async loadPlans(refresh: boolean) {
      if (refresh && this.data.refreshing) return
      this.setData(refresh ? { refreshing: true } : { loading: true, error: '' })
      try {
        const response = await apiRequest<AiResponse>(`/api/ai/plans?refresh=${refresh}`)
        this.setData({
          tabs: response.tabs,
          insightTitle: response.insight.title,
          insightDescription: response.insight.description,
          plans: response.plans.map((plan) => ({
            ...plan,
            budget: formatMoney(plan.budgetAmount),
            saved: false,
          })),
          refreshText: refresh ? '已更新推荐' : '换一批推荐',
          loading: false,
          refreshing: false,
        })
      } catch (error) {
        const message = error instanceof Error ? error.message : '推荐加载失败'
        if (refresh) {
          this.setData({ refreshing: false })
          wx.showToast({ title: message, icon: 'none' })
        } else {
          this.setData({ loading: false, error: message })
        }
      }
    },
    changeTab(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      this.setData({ activeTab: key })
    },
    async refresh() {
      await this.loadPlans(true)
    },
    async viewPlan(event: WechatMiniprogram.TouchEvent) {
      const id = event.currentTarget.dataset.id as string
      if (this.data.savingId || this.data.plans.find((plan) => plan.id === id)?.saved) return
      this.setData({ savingId: id })
      try {
        await apiRequest(`/api/ai/plans/${id}/save`, 'POST')
        this.setData({
          plans: this.data.plans.map((plan) => (plan.id === id ? { ...plan, saved: true } : plan)),
        })
        wx.showToast({ title: '方案已收藏', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '收藏失败', icon: 'none' })
      } finally {
        this.setData({ savingId: '' })
      }
    },
    openMatch() {
      wx.redirectTo({ url: '/pages/match/match' })
    },
  },
})

function formatMoney(cents: number): string {
  return `¥${(cents / 100).toLocaleString('zh-CN')}`
}
