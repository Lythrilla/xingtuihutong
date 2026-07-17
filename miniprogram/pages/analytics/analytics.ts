import { apiRequest, goTo } from '../../utils/api'

export {}

interface AnalyticsResponse {
  metrics: Array<{
    key: string
    label: string
    value: number
    displayValue: string
    change: number
  }>
  trend: Array<{
    date: string
    label: string
    matches: number
    connections: number
  }>
  funnel: Array<{
    label: string
    value: number
    conversion: number
  }>
  channels: Array<{
    label: string
    value: number
    percent: number
  }>
  opportunity: {
    score: number
    title: string
    description: string
    action: string
  }
}

Component({
  data: {
    loading: true,
    error: '',
    metrics: [] as AnalyticsResponse['metrics'],
    trend: [] as Array<AnalyticsResponse['trend'][number] & { matchHeight: number; connectionHeight: number }>,
    funnel: [] as Array<AnalyticsResponse['funnel'][number] & { width: number }>,
    channels: [] as AnalyticsResponse['channels'],
    opportunity: {} as AnalyticsResponse['opportunity'],
  },
  lifetimes: {
    async attached() {
      await this.loadAnalytics()
    },
  },
  methods: {
    retry() {
      return this.loadAnalytics()
    },
    async loadAnalytics() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<AnalyticsResponse>('/api/analytics/overview')
        const maximum = Math.max(
          1,
          ...response.trend.flatMap((item) => [item.matches, item.connections]),
        )
        const funnelMaximum = Math.max(1, ...response.funnel.map((item) => item.value))
        this.setData({
          ...response,
          trend: response.trend.map((item) => ({
            ...item,
            matchHeight: Math.max(6, Math.round((item.matches / maximum) * 112)),
            connectionHeight: Math.max(6, Math.round((item.connections / maximum) * 112)),
          })),
          funnel: response.funnel.map((item) => ({
            ...item,
            width: Math.max(4, Math.round((item.value / funnelMaximum) * 100)),
          })),
          loading: false,
        })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '数据洞察加载失败',
        })
      }
    },
    askAgent() {
      goTo('/pages/ai/ai')
    },
    openPlaza() {
      goTo('/pages/plaza/plaza')
    },
  },
})
