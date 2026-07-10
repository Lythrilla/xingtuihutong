import { apiRequest } from '../../utils/api'

export {}

interface Metric {
  value: string
  label: string
}

interface Recommendation {
  id: string
  avatar: string
  avatarClass: string
  title: string
  subtitle: string
  score: number
  price: string
}

interface HomeResponse {
  headerSubtitle: string
  metrics: Metric[]
  recommendations: Recommendation[]
}

Component({
  data: {
    headerSubtitle: '',
    metrics: [] as Metric[],
    recommendations: [] as Recommendation[],
  },
  lifetimes: {
    async attached() {
      try {
        const response = await apiRequest<HomeResponse>('/api/home')
        this.setData(response)
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '首页加载失败', icon: 'none' })
      }
    },
  },
  methods: {
    openAI() {
      wx.redirectTo({ url: '/pages/ai/ai' })
    },
    openMatch() {
      wx.redirectTo({ url: '/pages/match/match' })
    },
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
  },
})
