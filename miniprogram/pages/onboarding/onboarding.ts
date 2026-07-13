import { apiRequest } from '../../utils/api'

export {}

const app = getApp<IAppOption>()

type OnboardingStatus = 'draft' | 'pending' | 'approved' | 'rejected'

interface OnboardingApplication {
  entityName: string
  contactName: string
  contactMethod: string
  description: string
  tags: string[]
  workTitle: string
  workUrl: string
  audienceSize: string
  cooperationBudget: string
  status: OnboardingStatus
  reviewNote: string
}

interface OnboardingResponse {
  role: 'provider' | 'client'
  status: OnboardingStatus
  reviewNote: string
  application: OnboardingApplication | null
}

interface TagOption {
  label: string
  selected: boolean
}

const creatorTags = ['流行', '说唱', '民谣', '电子', '国风', '摇滚']
const providerTags = ['短视频宣发', '达人矩阵', '校园推广', '音乐媒体', '品牌联动', '直播推广']

Component({
  data: {
    loading: true,
    error: '',
    submitting: false,
    editing: true,
    role: 'client' as 'provider' | 'client',
    isCreator: true,
    status: 'draft' as OnboardingStatus,
    reviewNote: '',
    entityName: '',
    contactName: '',
    contactMethod: '',
    description: '',
    workTitle: '',
    workUrl: '',
    audienceSize: '',
    cooperationBudget: '',
    selectedTags: [] as string[],
    tagOptions: [] as TagOption[],
  },
  lifetimes: {
    attached() {
      void this.loadApplication()
    },
  },
  methods: {
    retry() {
      return this.loadApplication()
    },
    async loadApplication() {
      this.setData({ loading: true, error: '' })
      try {
        const response = await apiRequest<OnboardingResponse>('/api/onboarding')
        app.globalData.role = response.role
        app.globalData.onboardingStatus = response.status
        wx.setStorageSync('starconnect-onboarding-status', response.status)
        const application = response.application
        const selectedTags = application?.tags ?? []
        const isCreator = response.role === 'client'
        const options = isCreator ? creatorTags : providerTags
        this.setData({
          role: response.role,
          isCreator,
          status: response.status,
          reviewNote: response.reviewNote || application?.reviewNote || '',
          entityName: application?.entityName ?? '',
          contactName: application?.contactName ?? '',
          contactMethod: application?.contactMethod ?? '',
          description: application?.description ?? '',
          workTitle: application?.workTitle ?? '',
          workUrl: application?.workUrl ?? '',
          audienceSize: application?.audienceSize ?? '',
          cooperationBudget: application?.cooperationBudget ?? '',
          selectedTags,
          tagOptions: options.map((label) => ({ label, selected: selectedTags.includes(label) })),
          editing: response.status === 'draft' || response.status === 'rejected',
          loading: false,
        })
      } catch (error) {
        this.setData({
          loading: false,
          error: error instanceof Error ? error.message : '入驻资料加载失败',
        })
      }
    },
    updateField(event: WechatMiniprogram.Input) {
      const field = event.currentTarget.dataset.field as string
      this.setData({ [field]: event.detail.value })
    },
    toggleTag(event: WechatMiniprogram.TouchEvent) {
      const label = event.currentTarget.dataset.label as string
      const selectedTags = this.data.selectedTags.includes(label)
        ? this.data.selectedTags.filter((tag) => tag !== label)
        : [...this.data.selectedTags, label]
      this.setData({
        selectedTags,
        tagOptions: this.data.tagOptions.map((option) => ({
          ...option,
          selected: selectedTags.includes(option.label),
        })),
      })
    },
    editApplication() {
      if (this.data.status === 'approved') {
        wx.showModal({
          title: '更新后需重新审核',
          content: '提交新资料后，当前公开主页会暂时下线，审核通过后重新展示。',
          confirmText: '继续更新',
          success: (result) => {
            if (result.confirm) this.setData({ editing: true })
          },
        })
        return
      }
      this.setData({ editing: true })
    },
    goHome() {
      wx.redirectTo({ url: '/pages/home/home' })
    },
    async submitApplication() {
      if (this.data.submitting) return
      const required = [
        this.data.entityName,
        this.data.contactName,
        this.data.contactMethod,
        this.data.description,
      ]
      if (required.some((value) => !value.trim())) {
        wx.showToast({ title: '请完整填写必填资料', icon: 'none' })
        return
      }
      if (this.data.isCreator && (!this.data.workTitle.trim() || !this.data.workUrl.trim())) {
        wx.showToast({ title: '请填写代表作品和作品链接', icon: 'none' })
        return
      }
      if (!this.data.selectedTags.length) {
        wx.showToast({ title: '请至少选择一个标签', icon: 'none' })
        return
      }
      this.setData({ submitting: true })
      try {
        const response = await apiRequest<OnboardingResponse>('/api/onboarding', 'PUT', {
          entityName: this.data.entityName.trim(),
          contactName: this.data.contactName.trim(),
          contactMethod: this.data.contactMethod.trim(),
          description: this.data.description.trim(),
          tags: this.data.selectedTags,
          workTitle: this.data.workTitle.trim(),
          workUrl: this.data.workUrl.trim(),
          audienceSize: this.data.audienceSize.trim(),
          cooperationBudget: this.data.cooperationBudget.trim(),
        })
        this.setData({
          status: response.status,
          reviewNote: response.reviewNote,
          editing: false,
        })
        app.globalData.onboardingStatus = response.status
        wx.setStorageSync('starconnect-onboarding-status', response.status)
        wx.showToast({ title: '入驻申请已提交', icon: 'success' })
      } catch (error) {
        wx.showToast({ title: error instanceof Error ? error.message : '提交失败', icon: 'none' })
      } finally {
        this.setData({ submitting: false })
      }
    },
  },
})
