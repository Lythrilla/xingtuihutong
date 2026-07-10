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
    loading: true,
    listLoading: false,
    error: '',
    activeType: 'all',
    query: '',
    types: [] as FilterOption[],
    entries: [] as PlazaEntry[],
    visibleEntries: [] as PlazaEntry[],
    connectingId: '',
  },
  lifetimes: {
    async attached() {
      await this.loadEntries('all')
    },
  },
  methods: {
    retry() {
      return this.loadEntries(this.data.activeType || 'all')
    },
    handleEmptyAction() {
      if (this.data.query) {
        this.clearSearch()
        return
      }
      return this.retry()
    },
    async loadEntries(type: string) {
      const initial = !this.data.types.length
      this.setData({
        loading: initial,
        listLoading: !initial,
        error: '',
      })
      try {
        const response = await apiRequest<PlazaResponse>(`/api/plaza?type=${type}`)
        this.setData({
          activeType: type,
          types: response.types,
          entries: response.entries,
          loading: false,
          listLoading: false,
        })
        this.filterEntries(this.data.query)
      } catch (error) {
        this.setData({
          loading: false,
          listLoading: false,
          error: error instanceof Error ? error.message : '广场加载失败',
        })
      }
    },
    async changeType(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      if (key === this.data.activeType || this.data.listLoading) return
      await this.loadEntries(key)
    },
    updateSearch(event: WechatMiniprogram.Input) {
      const query = event.detail.value
      this.setData({ query })
      this.filterEntries(query)
    },
    clearSearch() {
      this.setData({ query: '' })
      this.filterEntries('')
    },
    filterEntries(query: string) {
      const keyword = query.trim().toLocaleLowerCase()
      const visibleEntries = keyword
        ? this.data.entries.filter((entry) =>
            [entry.name, entry.identity, entry.description, ...entry.tags].some((value) =>
              value.toLocaleLowerCase().includes(keyword),
            ),
          )
        : this.data.entries
      this.setData({ visibleEntries })
    },
    async connect(event: WechatMiniprogram.TouchEvent) {
      const partnerId = event.currentTarget.dataset.id as string
      if (this.data.connectingId) return
      this.setData({ connectingId: partnerId })
      try {
        const response = await apiRequest<{ conversationId: string; partnerName: string }>(
          '/api/plaza/connect',
          'POST',
          { partnerId },
        )
        wx.navigateTo({
          url: `/pages/conversation/conversation?id=${encodeURIComponent(response.conversationId)}&name=${encodeURIComponent(response.partnerName)}`,
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '发起沟通失败', icon: 'none' })
      } finally {
        this.setData({ connectingId: '' })
      }
    },
  },
})
