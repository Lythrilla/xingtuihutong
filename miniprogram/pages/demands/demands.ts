import { apiRequest } from '../../utils/api'

export {}

type DemandStatus = 'open' | 'following' | 'completed' | 'closed'
type ProposalStatus = 'pending' | 'accepted' | 'rejected' | 'withdrawn'

interface Proposal {
  id: string
  providerUserId: string
  providerName: string
  providerAvatar: string
  amount: number
  cycle: string
  deliverables: string
  message: string
  status: ProposalStatus
  createdAt: string
  updatedAt: string
  amountLabel?: string
  statusLabel?: string
}

interface Demand {
  id: string
  creatorName: string
  creatorAvatar: string
  songName: string
  targetKeys: string[]
  targetLabels: string[]
  budgetLabel: string
  goal: string
  cycle: string
  status: DemandStatus
  proposalCount: number
  proposals: Proposal[]
  createdAt: string
  targetText?: string
  statusLabel?: string
  createdAtLabel?: string
  creatorAvatarText?: string
  ownProposal?: Proposal
}

interface DemandBoardResponse {
  role: 'provider' | 'client'
  demands: Demand[]
}

Component({
  data: {
    loading: true,
    refreshing: false,
    error: '',
    role: 'provider' as 'provider' | 'client',
    isCreator: false,
    demands: [] as Demand[],
    sheetOpen: false,
    actionLoading: false,
    activeDemandId: '',
    amountInput: '',
    cycleInput: '',
    deliverablesInput: '',
    messageInput: '',
    canSubmitProposal: false,
  },
  lifetimes: {
    attached() {
      void this.loadDemands()
    },
  },
  pageLifetimes: {
    show() {
      if (!this.data.loading && !this.data.refreshing) void this.loadDemands(false)
    },
  },
  methods: {
    retry() {
      return this.loadDemands()
    },
    async loadDemands(initial = true) {
      this.setData(initial ? { loading: true, error: '' } : { refreshing: true })
      try {
        const response = await apiRequest<DemandBoardResponse>('/api/demands')
        const demands = response.demands.map((demand) => {
          const proposals = demand.proposals.map((proposal) => ({
            ...proposal,
            amountLabel: formatMoney(proposal.amount),
            statusLabel: proposalStatusLabel(proposal.status),
          }))
          return {
            ...demand,
            proposals,
            ownProposal: response.role === 'provider' ? proposals[0] : undefined,
            targetText: demand.targetLabels.join(' · '),
            statusLabel: demandStatusLabel(demand.status),
            createdAtLabel: formatDate(demand.createdAt),
            creatorAvatarText: demand.creatorAvatar || demand.creatorName.slice(0, 1),
          }
        })
        this.setData({
          role: response.role,
          isCreator: response.role === 'client',
          demands,
          loading: false,
          refreshing: false,
        })
      } catch (error) {
        const message = error instanceof Error ? error.message : '需求列表加载失败'
        if (initial) {
          this.setData({ loading: false, error: message })
        } else {
          this.setData({ refreshing: false })
          wx.showToast({ title: message, icon: 'none' })
        }
      }
    },
    openProposalSheet(event: WechatMiniprogram.TouchEvent) {
      const demandId = event.currentTarget.dataset.id as string
      const demand = this.data.demands.find((item) => item.id === demandId)
      if (!demand || demand.status !== 'open') return
      const proposal = demand.ownProposal
      this.setData({
        sheetOpen: true,
        activeDemandId: demandId,
        amountInput: proposal ? (proposal.amount / 100).toFixed(0) : '',
        cycleInput: proposal?.cycle || demand.cycle,
        deliverablesInput: proposal?.deliverables || '',
        messageInput: proposal?.message || '',
      })
      this.updateProposalState()
    },
    closeSheet() {
      if (this.data.actionLoading) return
      this.setData({ sheetOpen: false, activeDemandId: '' })
    },
    preventClose() {},
    updateAmount(event: WechatMiniprogram.Input) {
      this.setData({ amountInput: event.detail.value })
      this.updateProposalState()
    },
    updateCycle(event: WechatMiniprogram.Input) {
      this.setData({ cycleInput: event.detail.value })
      this.updateProposalState()
    },
    updateDeliverables(event: WechatMiniprogram.Input) {
      this.setData({ deliverablesInput: event.detail.value })
      this.updateProposalState()
    },
    updateMessage(event: WechatMiniprogram.Input) {
      this.setData({ messageInput: event.detail.value })
    },
    updateProposalState() {
      const amount = Number(this.data.amountInput)
      this.setData({
        canSubmitProposal:
          Number.isFinite(amount) &&
          amount >= 1 &&
          this.data.cycleInput.trim().length >= 2 &&
          this.data.deliverablesInput.trim().length >= 4,
      })
    },
    async submitProposal() {
      if (!this.data.canSubmitProposal || this.data.actionLoading) return
      this.setData({ actionLoading: true })
      try {
        await apiRequest(`/api/demands/${this.data.activeDemandId}/proposals`, 'POST', {
          amount: Math.round(Number(this.data.amountInput) * 100),
          cycle: this.data.cycleInput.trim(),
          deliverables: this.data.deliverablesInput.trim(),
          message: this.data.messageInput.trim(),
        })
        this.setData({ sheetOpen: false, activeDemandId: '' })
        await this.loadDemands(false)
        wx.showToast({ title: '报价已提交', icon: 'success' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '报价提交失败', icon: 'none' })
      } finally {
        this.setData({ actionLoading: false })
      }
    },
    withdrawProposal(event: WechatMiniprogram.TouchEvent) {
      const demandId = event.currentTarget.dataset.id as string
      wx.showModal({
        title: '撤回当前报价',
        content: '撤回后仍可在需求开放期间重新报价。',
        confirmText: '确认撤回',
        success: (result) => {
          if (result.confirm) void this.confirmWithdraw(demandId)
        },
      })
    },
    async confirmWithdraw(demandId: string) {
      try {
        await apiRequest(`/api/demands/${demandId}/proposals`, 'DELETE')
        await this.loadDemands(false)
        wx.showToast({ title: '报价已撤回', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '撤回失败', icon: 'none' })
      }
    },
    acceptProposal(event: WechatMiniprogram.TouchEvent) {
      const proposalId = event.currentTarget.dataset.id as string
      const proposal = this.findProposal(proposalId)
      if (!proposal) return
      wx.showModal({
        title: `接受 ${proposal.providerName} 的报价`,
        content: `确认以 ${proposal.amountLabel}、${proposal.cycle} 推进合作？其他待选报价将自动结束。`,
        confirmText: '确认接单',
        success: (result) => {
          if (result.confirm) void this.confirmAccept(proposalId)
        },
      })
    },
    async confirmAccept(proposalId: string) {
      if (this.data.actionLoading) return
      this.setData({ actionLoading: true })
      try {
        const response = await apiRequest<{ conversationId: string; providerName: string }>(
          `/api/demands/proposals/${proposalId}/accept`,
          'POST',
        )
        wx.showToast({ title: '已建立合作会话', icon: 'success' })
        setTimeout(() => {
          wx.redirectTo({
            url: `/pages/conversation/conversation?id=${encodeURIComponent(response.conversationId)}&name=${encodeURIComponent(response.providerName)}`,
          })
        }, 400)
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '接单失败', icon: 'none' })
      } finally {
        this.setData({ actionLoading: false })
      }
    },
    closeDemand(event: WechatMiniprogram.TouchEvent) {
      const demandId = event.currentTarget.dataset.id as string
      wx.showModal({
        title: '关闭推广需求',
        content: '关闭后将结束所有待选报价，且不能继续接收新报价。',
        confirmText: '确认关闭',
        success: (result) => {
          if (result.confirm) void this.confirmCloseDemand(demandId)
        },
      })
    },
    async confirmCloseDemand(demandId: string) {
      try {
        await apiRequest(`/api/demands/${demandId}/close`, 'POST')
        await this.loadDemands(false)
        wx.showToast({ title: '需求已关闭', icon: 'none' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '关闭失败', icon: 'none' })
      }
    },
    openMessages() {
      wx.redirectTo({ url: '/pages/messages/messages' })
    },
    publishDemand() {
      wx.redirectTo({ url: '/pages/match/match' })
    },
    findProposal(proposalId: string): Proposal | undefined {
      for (const demand of this.data.demands) {
        const proposal = demand.proposals.find((item) => item.id === proposalId)
        if (proposal) return proposal
      }
      return undefined
    },
  },
})

function demandStatusLabel(status: DemandStatus): string {
  return {
    open: '报价中',
    following: '合作推进中',
    completed: '已完成',
    closed: '已关闭',
  }[status]
}

function proposalStatusLabel(status: ProposalStatus): string {
  return {
    pending: '待选择',
    accepted: '已接受',
    rejected: '未入选',
    withdrawn: '已撤回',
  }[status]
}

function formatMoney(amount: number): string {
  return `¥${(amount / 100).toLocaleString('zh-CN', { maximumFractionDigits: 2 })}`
}

function formatDate(value: string): string {
  const normalized = value.includes('T') ? value : `${value.replace(' ', 'T')}Z`
  const date = new Date(normalized)
  if (Number.isNaN(date.getTime())) return value.slice(0, 10)
  return `${date.getMonth() + 1}月${date.getDate()}日`
}
