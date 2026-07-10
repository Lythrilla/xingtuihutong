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
}

interface AiResponse {
  insight: {
    title: string
    description: string
  }
  tabs: Tab[]
  plans: Omit<Plan, 'budget'>[]
}

Component({
  data: {
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
    async loadPlans(refresh: boolean) {
      try {
        const response = await apiRequest<AiResponse>(`/api/ai/plans?refresh=${refresh}`)
        this.setData({
          tabs: response.tabs,
          insightTitle: response.insight.title,
          insightDescription: response.insight.description,
          plans: response.plans.map((plan) => ({
            ...plan,
            budget: formatMoney(plan.budgetAmount),
          })),
          refreshText: refresh ? '已更新推荐' : '换一批推荐',
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '推荐加载失败', icon: 'none' })
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
      try {
        await apiRequest(`/api/ai/plans/${id}/save`, 'POST')
        wx.showToast({ title: '方案已收藏', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '收藏失败', icon: 'none' })
      }
    },
  },
})

function formatMoney(cents: number): string {
  return `¥${(cents / 100).toLocaleString('zh-CN')}`
}
