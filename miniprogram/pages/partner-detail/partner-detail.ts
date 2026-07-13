import { apiRequest } from '../../utils/api'

export {}

interface Partner {
  id: string
  partnerType: 'provider' | 'client'
  avatar: string
  avatarClass: string
  name: string
  identity: string
  description: string
  tags: string[]
  matchScore: number
  resultText: string
}

interface PartnerDetailResponse {
  partner: Partner
  verificationItems: string[]
  contactPreview: string
  contactAvailable: boolean
  reviewedAt: string
  role: 'provider' | 'client'
  onboardingStatus: 'draft' | 'pending' | 'approved' | 'rejected'
  canConnect: boolean
}

Component({
  data: {
    loading: true,
    error: '',
    partnerId: '',
    partner: null as Partner | null,
    verificationItems: [] as string[],
    contactPreview: '',
    contactAvailable: false,
    reviewedAt: '',
    isCreator: true,
    canConnect: false,
    favorite: false,
    showAccessSheet: false,
    selectedAccess: 'member' as 'member' | 'single',
    connecting: false,
  },
  methods: {
    onLoad(options: Record<string, string | undefined>) {
      const partnerId = options.id || ''
      this.setData({ partnerId })
      this.loadDetail()
    },
    retry() {
      return this.loadDetail()
    },
    async loadDetail() {
      if (!this.data.partnerId) {
        this.setData({ loading: false, error: '合作方信息不完整，请返回后重试' })
        return
      }
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<PartnerDetailResponse>(
          `/api/partners/${encodeURIComponent(this.data.partnerId)}`,
        )
        this.setData({
          ...response,
          isCreator: response.role === 'client',
          favorite: getFavoriteIds().includes(response.partner.id),
          reviewedAt: formatReviewedAt(response.reviewedAt),
          loading: false,
        })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '合作方详情加载失败',
        })
      }
    },
    toggleFavorite() {
      const partner = this.data.partner
      if (!partner) return
      const favorite = !this.data.favorite
      const favoriteIds = getFavoriteIds()
      const nextIds = favorite
        ? Array.from(new Set([...favoriteIds, partner.id]))
        : favoriteIds.filter((id) => id !== partner.id)
      wx.setStorageSync('starconnect-favorite-partners', nextIds)
      this.setData({ favorite })
      wx.showToast({ title: favorite ? '已收藏合作伙伴' : '已取消收藏', icon: 'none' })
    },
    openAccess() {
      if (!this.data.contactAvailable) {
        wx.showToast({ title: '该主页暂未配置可解锁联系方式', icon: 'none' })
        return
      }
      if (!this.data.canConnect) {
        wx.showModal({
          title: '完成入驻后再联系',
          content: '审核通过前可以查看公开资料，但不能解锁联系方式或发起合作。',
          confirmText: '查看入驻',
          success: (result) => {
            if (result.confirm) wx.redirectTo({ url: '/pages/onboarding/onboarding' })
          },
        })
        return
      }
      this.setData({ showAccessSheet: true })
    },
    closeAccess() {
      if (this.data.connecting) return
      this.setData({ showAccessSheet: false })
    },
    preventClose() {},
    chooseAccess(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedAccess: event.currentTarget.dataset.mode as 'member' | 'single' })
    },
    attemptUnlock() {
      wx.showModal({
        title: '当前不会扣款',
        content: '会员额度与微信支付仍在接入，本次不会扣除费用。你可以先创建站内会话，支付能力完成合规配置后再解锁外部联系方式。',
        confirmText: '站内沟通',
        cancelText: '暂不联系',
        success: (result) => {
          if (result.confirm) void this.connect()
        },
      })
    },
    async connect() {
      const partner = this.data.partner
      if (!partner || this.data.connecting) return
      this.setData({ connecting: true })
      try {
        const response = await apiRequest<{ conversationId: string; partnerName: string }>(
          '/api/plaza/connect',
          'POST',
          { partnerId: partner.id },
        )
        this.setData({ showAccessSheet: false })
        wx.navigateTo({
          url: `/pages/conversation/conversation?id=${encodeURIComponent(response.conversationId)}&name=${encodeURIComponent(response.partnerName)}`,
        })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '发起沟通失败', icon: 'none' })
      } finally {
        this.setData({ connecting: false })
      }
    },
  },
})

function getFavoriteIds(): string[] {
  const stored = wx.getStorageSync('starconnect-favorite-partners') as unknown
  return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === 'string') : []
}

function formatReviewedAt(value: string): string {
  if (!value) return '审核记录可追溯'
  return `资料于 ${value.slice(0, 10)} 完成审核`
}
