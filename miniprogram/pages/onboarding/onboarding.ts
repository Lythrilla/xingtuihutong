import { apiRequest, uploadWorkFile } from '../../utils/api'

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
  workFileUrl: string
  workFileName: string
  verificationItems: string[]
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

const verificationOptions = [
  { key: 'ownership', label: '我确认拥有该作品或已获得完整授权' },
  { key: 'publishable', label: '我确认平台可将作品用于入驻审核' },
  { key: 'authentic', label: '我确认提交的身份与作品信息真实有效' },
]

const creatorTags = ['流行', '说唱', '民谣', '电子', '国风', '摇滚']
const providerTags = ['短视频宣发', '达人矩阵', '校园推广', '音乐媒体', '品牌联动', '直播推广']

Component({
  data: {
    loading: true,
    error: '',
    submitting: false,
    uploading: false,
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
    workFileUrl: '',
    workFileName: '',
    audienceSize: '',
    cooperationBudget: '',
    selectedTags: [] as string[],
    tagOptions: [] as TagOption[],
    verificationItems: [] as string[],
    verificationOptions: verificationOptions.map((item) => ({ ...item, selected: false })),
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
          workFileUrl: application?.workFileUrl ?? '',
          workFileName: application?.workFileName ?? '',
          audienceSize: application?.audienceSize ?? '',
          cooperationBudget: application?.cooperationBudget ?? '',
          selectedTags,
          tagOptions: options.map((label) => ({ label, selected: selectedTags.includes(label) })),
          verificationItems: application?.verificationItems ?? [],
          verificationOptions: verificationOptions.map((item) => ({
            ...item,
            selected: (application?.verificationItems ?? []).includes(item.key),
          })),
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
    toggleVerification(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      const verificationItems = this.data.verificationItems.includes(key)
        ? this.data.verificationItems.filter((item) => item !== key)
        : [...this.data.verificationItems, key]
      this.setData({
        verificationItems,
        verificationOptions: this.data.verificationOptions.map((item) => ({
          ...item,
          selected: verificationItems.includes(item.key),
        })),
      })
    },
    chooseWork() {
      if (this.data.uploading) return
      wx.chooseMessageFile({
        count: 1,
        type: 'file',
        extension: ['mp3', 'wav', 'm4a', 'mp4', 'mov', 'jpg', 'jpeg', 'png'],
        success: async (result) => {
          const file = result.tempFiles[0]
          if (!file) return
          if (file.size > 30 * 1024 * 1024) {
            wx.showToast({ title: '作品文件需小于 30MB', icon: 'none' })
            return
          }
          this.setData({ uploading: true })
          try {
            const uploaded = await uploadWorkFile(file.path)
            this.setData({
              workFileUrl: uploaded.url,
              workFileName: uploaded.fileName || file.name,
              workTitle: this.data.workTitle || file.name.replace(/\.[^.]+$/, ''),
            })
            wx.showToast({ title: '作品已上传', icon: 'success' })
          } catch (error) {
            wx.showToast({
              title: error instanceof Error ? error.message : '作品上传失败',
              icon: 'none',
            })
          } finally {
            this.setData({ uploading: false })
          }
        },
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
    goBackToIndex() {
      wx.redirectTo({ url: '/pages/index/index' })
    },
    async submitApplication() {
      if (this.data.submitting) return
      const required = [this.data.entityName, this.data.contactName, this.data.contactMethod]
      if (!this.data.isCreator) required.push(this.data.description)
      if (required.some((value) => !value.trim())) {
        wx.showToast({ title: '请完整填写必填资料', icon: 'none' })
        return
      }
      if (this.data.isCreator && !this.data.workFileUrl && !this.data.workUrl) {
        wx.showToast({ title: '请上传一份代表作品', icon: 'none' })
        return
      }
      if (this.data.isCreator && this.data.verificationItems.length !== verificationOptions.length) {
        wx.showToast({ title: '请确认全部作品声明', icon: 'none' })
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
          workFileUrl: this.data.workFileUrl,
          workFileName: this.data.workFileName,
          verificationItems: this.data.verificationItems,
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
