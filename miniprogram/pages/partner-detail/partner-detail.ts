import { apiRequest, unlockPartnerContact } from '../../utils/api'

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

interface PartnerDetailPageData {
  loading: boolean
  error: string
  partnerId: string
  partner: Partner | null
  verificationItems: string[]
  contactPreview: string
  contactMethod: string
  contactAvailable: boolean
  unlocked: boolean
  reviewedAt: string
  isCreator: boolean
  canConnect: boolean
  favorite: boolean
  showAccessSheet: boolean
  selectedAccess: 'member' | 'single'
  connecting: boolean
  unlocking: boolean
}

Page<PartnerDetailPageData, WechatMiniprogram.IAnyObject>({
  data: {
    loading: true,
    error: '',
    partnerId: '',
    partner: null,
    verificationItems: [],
    contactPreview: '',
    contactMethod: '',
    contactAvailable: false,
    unlocked: false,
    reviewedAt: '',
    isCreator: true,
    canConnect: false,
    favorite: false,
    showAccessSheet: false,
    selectedAccess: 'single',
    connecting: false,
    unlocking: false,
  },
  onLoad(options: Record<string, string | undefined>) {
    const partnerId = options.id || ''
    this.setData({ partnerId })
    this.loadDetail()
  },
  onShow() {
    if (this.data.partnerId) {
      this.loadDetail()
    }
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
      const response = await apiRequest<{
        partner: Partner
        verificationItems: string[]
        contactPreview: string
        contactMethod: string
        contactAvailable: boolean
        unlocked: boolean
        reviewedAt: string
        role: 'provider' | 'client'
        onboardingStatus: 'draft' | 'pending' | 'approved' | 'rejected'
        canConnect: boolean
      }>(`/api/partners/${encodeURIComponent(this.data.partnerId)}`)
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
          if (result.confirm) wx.navigateTo({ url: '/pages/onboarding/onboarding' })
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
  async attemptUnlock() {
    if (this.data.unlocking || !this.data.partner) return
    if (this.data.selectedAccess === 'member') {
      this.setData({ showAccessSheet: false })
      wx.navigateTo({ url: '/pages/membership/membership' })
      return
    }
    this.setData({ unlocking: true })
    try {
      const result = await unlockPartnerContact(this.data.partner.id)
      this.setData({
        unlocked: true,
        contactMethod: result.contactMethod,
        showAccessSheet: false,
      })
      wx.showToast({ title: '联系方式已解锁', icon: 'success' })
    } catch (error) {
      const message = error instanceof Error ? error.message : '解锁失败'
      if (message.includes('余额不足')) {
        wx.showModal({
          title: '余额不足',
          content: '钱包余额不足以按次解锁，可前往开通会员或补充余额。',
          confirmText: '去开通会员',
          success: (res) => {
            if (res.confirm) wx.navigateTo({ url: '/pages/membership/membership' })
          },
        })
      } else {
        wx.showToast({ title: message, icon: 'none' })
      }
    } finally {
      this.setData({ unlocking: false })
    }
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
})

function getFavoriteIds(): string[] {
  const stored = wx.getStorageSync('starconnect-favorite-partners') as unknown
  return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === 'string') : []
}

function formatReviewedAt(value: string): string {
  if (!value) return '审核记录可追溯'
  return `资料于 ${value.slice(0, 10)} 完成审核`
}
