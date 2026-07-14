import { apiRequest } from '../../utils/api'

export {}

interface AgentMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  createdAt: string
}

interface ToolCall {
  id: string
  name: string
  label: string
  status: string
  input: Record<string, unknown>
  output: Record<string, unknown>
  durationMs: number
}

interface ToolDefinition {
  name: string
  label: string
  description: string
  mode: 'read' | 'write'
}

interface FunnelItem {
  label: string
  value: number
  width: number
}

interface ArtifactData {
  matches?: number
  conversations?: number
  savedPlans?: number
  revenueDisplay?: string
  partners?: Array<{
    id: string
    name: string
    identity: string
    description: string
    tags: string[]
    matchScore: number
  }>
  plans?: Array<{
    id: string
    title: string
    planType: string
    description: string
    tags: string[]
    budget: string
    score: number
  }>
  saved?: boolean
  created?: boolean
  status?: string
  conversationId?: string
  partnerName?: string
  actionId?: string
  planId?: string
}

interface Artifact {
  kind: 'metrics' | 'funnel' | 'partners' | 'plans' | 'action' | 'error'
  title: string
  summary: string
  data: ArtifactData | FunnelItem[]
}

interface ViewArtifact extends Artifact {
  funnel: FunnelItem[]
  payload: ArtifactData
  actionLabel: string
  actionNavigable: boolean
}

interface BootstrapResponse {
  sessionId: string
  engine: string
  role: 'provider' | 'client'
  messages: AgentMessage[]
  recentToolCalls: ToolCall[]
  suggestions: string[]
  tools: ToolDefinition[]
}

interface QueryResponse {
  sessionId: string
  message: AgentMessage
  toolCalls: ToolCall[]
  artifacts: Artifact[]
  suggestions: string[]
}

Component({
  data: {
    loading: true,
    error: '',
    thinking: false,
    sessionId: '',
    engine: '',
    role: 'client' as 'provider' | 'client',
    isCreator: true,
    input: '',
    messages: [] as AgentMessage[],
    toolCalls: [] as ToolCall[],
    artifacts: [] as ViewArtifact[],
    suggestions: [] as string[],
    tools: [] as ToolDefinition[],
    showTools: false,
    quickActions: [] as Array<{ title: string; description: string; prompt: string }>,
    scrollTarget: '',
  },
  lifetimes: {
    async attached() {
      await this.bootstrap()
    },
  },
  methods: {
    retry() {
      return this.bootstrap()
    },
    async bootstrap() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<BootstrapResponse>('/api/agent/bootstrap')
        const prefill = wx.getStorageSync('starconnect-ai-prefill') as string
        if (prefill) wx.removeStorageSync('starconnect-ai-prefill')
        this.setData({
          ...response,
          isCreator: response.role === 'client',
          quickActions:
            response.role === 'client'
              ? [
                  { title: '作品诊断', description: '查看推广与合作数据', prompt: '分析我的作品推广数据和当前合作漏斗' },
                  { title: '推广方案', description: '按预算推荐执行路径', prompt: '为我的新作品推荐一个推广方案并细分预算渠道' },
                  { title: '推广方筛选', description: '寻找匹配的服务团队', prompt: '帮我找适合新作品的推广方' },
                  { title: '合作跟进', description: '创建下一步执行任务', prompt: '检查我的合作进度并创建跟进任务' },
                ]
              : [
                  { title: '项目诊断', description: '查看服务与合作数据', prompt: '分析我的服务数据和合作转化漏斗' },
                  { title: '创作者筛选', description: '发现匹配的真实项目', prompt: '帮我找合适的创作者项目' },
                  { title: '方案建议', description: '生成合作执行方案', prompt: '推荐一个适合当前项目的推广方案' },
                  { title: '合作跟进', description: '创建下一步执行任务', prompt: '检查我的合作进度并创建跟进任务' },
                ],
          input: prefill || this.data.input,
          toolCalls: response.recentToolCalls,
          loading: false,
          scrollTarget: response.messages.length ? 'conversation-end' : '',
        })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : 'Agent 初始化失败',
        })
      }
    },
    updateInput(event: WechatMiniprogram.Input) {
      this.setData({ input: event.detail.value })
    },
    useSuggestion(event: WechatMiniprogram.TouchEvent) {
      const value = event.currentTarget.dataset.value as string
      this.setData({ input: value }, () => void this.send())
    },
    async send() {
      const message = this.data.input.trim()
      if (!message || this.data.thinking) return
      const pending: AgentMessage = {
        id: `pending-${Date.now()}`,
        role: 'user',
        content: message,
        createdAt: '',
      }
      this.setData({
        input: '',
        thinking: true,
        artifacts: [],
        toolCalls: [],
        messages: [...this.data.messages, pending],
        scrollTarget: 'conversation-end',
      })
      try {
        const response = await apiRequest<QueryResponse>('/api/agent/query', 'POST', {
          sessionId: this.data.sessionId,
          message,
        })
        this.setData({
          sessionId: response.sessionId,
          messages: [...this.data.messages, response.message],
          toolCalls: response.toolCalls,
          artifacts: response.artifacts.map(toViewArtifact),
          suggestions: response.suggestions,
          thinking: false,
          scrollTarget: 'conversation-end',
        })
      } catch (error) {
        this.setData({
          thinking: false,
          input: message,
          messages: this.data.messages.filter((item) => item.id !== pending.id),
        })
        wx.showToast({
          title: error instanceof Error ? error.message : 'Agent 执行失败',
          icon: 'none',
        })
      }
    },
    toggleTools() {
      this.setData({ showTools: !this.data.showTools })
    },
    openPartner(event: WechatMiniprogram.TouchEvent) {
      const id = event.currentTarget.dataset.id as string
      wx.setStorageSync('starconnect-agent-partner', id)
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
    openMessages() {
      wx.redirectTo({ url: '/pages/messages/messages' })
    },
    openMatch() {
      wx.redirectTo({ url: '/pages/match/match' })
    },
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
    openAnalytics() {
      wx.redirectTo({ url: '/pages/analytics/analytics' })
    },
    openAction(event: WechatMiniprogram.TouchEvent) {
      const conversationId = event.currentTarget.dataset.conversationId as string
      const partnerName = event.currentTarget.dataset.partnerName as string
      if (conversationId) {
        wx.navigateTo({
          url: `/pages/conversation/conversation?id=${encodeURIComponent(conversationId)}&name=${encodeURIComponent(partnerName || '合作会话')}`,
        })
        return
      }
      this.openMessages()
    },
  },
})

function toViewArtifact(artifact: Artifact): ViewArtifact {
  const maximum = Array.isArray(artifact.data)
    ? Math.max(1, ...artifact.data.map((item) => item.value))
    : 1
  const funnel = Array.isArray(artifact.data)
    ? artifact.data.map((item) => ({
        ...item,
        width: item.value > 0 ? Math.max(4, Math.round((item.value / maximum) * 100)) : 4,
      }))
    : []
  const payload = Array.isArray(artifact.data) ? {} : artifact.data
  const actionNavigable = Boolean(payload.conversationId || payload.actionId)
  const actionLabel = payload.conversationId
    ? '前往合作会话'
    : payload.actionId
      ? '查看跟进通知'
      : payload.saved
        ? payload.created === false
          ? '方案已在收藏中'
          : '方案已加入收藏'
        : '操作已完成'
  let title = artifact.title.replace(/Agent/gi, '智能体')
  if (title === '候选合作伙伴') {
    title = '智能推荐'
  }

  return {
    ...artifact,
    title,
    funnel,
    payload,
    actionLabel,
    actionNavigable,
  }
}
