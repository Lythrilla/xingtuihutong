import { getMembershipPlans, purchaseMembership } from '../../utils/api'

export {}

interface PlanView {
  key: string
  name: string
  description: string
  price: string
  badge: string
}

Component({
  data: {
    selectedPlan: 'monthly',
    purchasing: false,
    loading: true,
    balance: '¥0.00',
    activeUntil: '',
    plans: [] as PlanView[],
  },
  lifetimes: {
    attached() {
      void this.loadPlans()
    },
  },
  methods: {
    async loadPlans() {
      this.setData({ loading: true })
      try {
        const response = await getMembershipPlans()
        const plans = response.plans.map((plan) => ({
          key: plan.key,
          name: plan.name,
          description: plan.description,
          price: `¥${(plan.price / 100).toFixed(2)}`,
          badge: plan.key === 'monthly' ? '推荐' : '',
        }))
        const activeUntil = response.activeUntil
          ? `会员有效期至 ${response.activeUntil.slice(0, 10)}`
          : ''
        this.setData({
          plans,
          balance: `¥${(response.balance / 100).toFixed(2)}`,
          activeUntil,
          loading: false,
        })
      } catch (error) {
        wx.showToast({
          title: error instanceof Error ? error.message : '套餐加载失败',
          icon: 'none',
        })
        this.setData({ loading: false })
      }
    },
    selectPlan(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedPlan: event.currentTarget.dataset.key as string })
    },
    purchase() {
      if (this.data.purchasing) return
      const plan = this.data.selectedPlan
      const planView = this.data.plans.find((item) => item.key === plan)
      if (!planView) return
      wx.showModal({
        title: '确认开通会员',
        content: `使用钱包余额购买 ${planView.name}（${planView.price}），立即生效。`,
        confirmText: '确认购买',
        success: (result) => {
          if (!result.confirm) return
          void this.doPurchase(plan)
        },
      })
    },
    async doPurchase(plan: string) {
      this.setData({ purchasing: true })
      try {
        const result = await purchaseMembership(plan)
        wx.showToast({ title: '会员开通成功', icon: 'success' })
        this.setData({
          activeUntil: result.activeUntil
            ? `会员有效期至 ${result.activeUntil.slice(0, 10)}`
            : '',
          balance: `¥${(result.balance / 100).toFixed(2)}`,
        })
      } catch (error) {
        wx.showToast({
          title: error instanceof Error ? error.message : '购买失败',
          icon: 'none',
        })
      } finally {
        this.setData({ purchasing: false })
      }
    },
    openRules() {
      wx.showModal({
        title: '权益保障规则',
        content: '同一合作方重复查看不重复扣费；联系方式无效可提交证据申诉，平台核实后退回次数或按次额度，不做原路退款。',
        showCancel: false,
        confirmText: '知道了',
      })
    },
  },
})
