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
  status?: string
}

interface Artifact {
  kind: 'metrics' | 'funnel' | 'partners' | 'plans' | 'action'
  title: string
  summary: string
  data: ArtifactData | FunnelItem[]
}

interface ViewArtifact extends Artifact {
  funnel: FunnelItem[]
  payload: ArtifactData
}

interface BootstrapResponse {
  sessionId: string
  engine: string
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
    input: '',
    messages: [] as AgentMessage[],
    toolCalls: [] as ToolCall[],
    artifacts: [] as ViewArtifact[],
    suggestions: [] as string[],
    tools: [] as ToolDefinition[],
    showTools: false,
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
        this.setData({
          ...response,
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
      this.setData({ input: value })
      void this.send()
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
    openInsights() {
      wx.redirectTo({ url: '/pages/analytics/analytics' })
    },
    openPartner(event: WechatMiniprogram.TouchEvent) {
      const id = event.currentTarget.dataset.id as string
      wx.setStorageSync('starconnect-agent-partner', id)
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
    openMessages() {
      wx.redirectTo({ url: '/pages/messages/messages' })
    },
  },
})

function toViewArtifact(artifact: Artifact): ViewArtifact {
  const funnel = Array.isArray(artifact.data)
    ? artifact.data.map((item, index) => ({ ...item, width: item.value > 0 ? 30 + index * 18 : 4 }))
    : []
  return {
    ...artifact,
    funnel,
    payload: Array.isArray(artifact.data) ? {} : artifact.data,
  }
}
