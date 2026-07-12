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
  favorite: boolean
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
    previousType: 'all',
    favoritesOnly: false,
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
      if (this.data.favoritesOnly) {
        this.toggleFavoritesOnly()
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
        const favoriteIds = getFavoriteIds()
        const entries = response.entries.map((entry) => ({
          ...entry,
          favorite: favoriteIds.includes(entry.id),
        }))
        this.setData({
          activeType: type,
          types: response.types,
          entries,
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
      this.setData({ previousType: key })
      await this.loadEntries(key)
    },
    async toggleLatest() {
      if (this.data.listLoading) return
      if (this.data.activeType === 'latest') {
        await this.loadEntries(this.data.previousType)
        return
      }
      this.setData({ previousType: this.data.activeType })
      await this.loadEntries('latest')
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
      const favoriteEntries = this.data.favoritesOnly
        ? this.data.entries.filter((entry) => entry.favorite)
        : this.data.entries
      const visibleEntries = keyword
        ? favoriteEntries.filter((entry) =>
            [entry.name, entry.identity, entry.description, ...entry.tags].some((value) =>
              value.toLocaleLowerCase().includes(keyword),
            ),
          )
        : favoriteEntries
      this.setData({ visibleEntries })
    },
    toggleFavoritesOnly() {
      this.setData({ favoritesOnly: !this.data.favoritesOnly })
      this.filterEntries(this.data.query)
    },
    toggleFavorite(event: WechatMiniprogram.TouchEvent) {
      const id = event.currentTarget.dataset.id as string
      const entries = this.data.entries.map((entry) =>
        entry.id === id ? { ...entry, favorite: !entry.favorite } : entry,
      )
      const favoriteIds = entries.filter((entry) => entry.favorite).map((entry) => entry.id)
      wx.setStorageSync('starconnect-favorite-partners', favoriteIds)
      this.setData({ entries })
      this.filterEntries(this.data.query)
      wx.showToast({
        title: favoriteIds.includes(id) ? '已收藏合作伙伴' : '已取消收藏',
        icon: 'none',
      })
    },
    openAI() {
      wx.redirectTo({ url: '/pages/ai/ai' })
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

function getFavoriteIds(): string[] {
  const stored = wx.getStorageSync('starconnect-favorite-partners') as unknown
  return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === 'string') : []
}
