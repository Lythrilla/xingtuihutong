import { apiRequest } from '../../utils/api'

export {}

interface FilterOption {
  key: string
  label: string
}

interface PlazaEntry {
  id: string
  partnerType: string
  avatar: string
  avatarClass: string
  name: string
  identity: string
  description: string
  tags: string[]
  matchScore: number
  resultText: string
}

interface PlazaResponse {
  types: FilterOption[]
  entries: PlazaEntry[]
}

Component({
  data: {
    activeType: 'all',
    types: [] as FilterOption[],
    visibleEntries: [] as PlazaEntry[],
  },
  lifetimes: {
    async attached() {
      await this.loadEntries('all')
    },
  },
  methods: {
    async loadEntries(type: string) {
      try {
        const response = await apiRequest<PlazaResponse>(`/api/plaza?type=${type}`)
        this.setData({
          activeType: type,
          types: response.types,
          visibleEntries: response.entries,
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '广场加载失败', icon: 'none' })
      }
    },
    async changeType(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      await this.loadEntries(key)
    },
    async connect(event: WechatMiniprogram.TouchEvent) {
      const partnerId = event.currentTarget.dataset.id as string
      try {
        const response = await apiRequest<{ partnerName: string }>('/api/plaza/connect', 'POST', {
          partnerId,
        })
        wx.showToast({ title: `已向${response.partnerName}发起沟通`, icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '发起沟通失败', icon: 'none' })
      }
    },
  },
})
