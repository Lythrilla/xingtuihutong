export {}

type PaymentStatus = 'success' | 'pending' | 'failed' | 'unavailable'

const CONTENT: Record<
  PaymentStatus,
  { symbol: string; title: string; description: string; action: string }
> = {
  success: {
    symbol: '✓',
    title: '支付成功，权益已到账',
    description: '服务端已确认微信支付结果，可返回合作方详情查看完整联系方式。',
    action: '返回查看联系方式',
  },
  pending: {
    symbol: '…',
    title: '支付结果确认中',
    description: '暂未收到最终支付通知，请勿重复付款。系统会继续查询订单状态。',
    action: '重新查询支付结果',
  },
  failed: {
    symbol: '!',
    title: '本次支付未完成',
    description: '没有获得联系权益，也不会消耗新人优惠资格。你可以返回后重新发起。',
    action: '返回重新选择',
  },
  unavailable: {
    symbol: '—',
    title: '微信支付尚未开放',
    description: '当前未创建订单、未扣款，也未发放权益。待价格、商户与合规配置完成后开放。',
    action: '返回权益中心',
  },
}

Component({
  data: {
    status: 'unavailable' as PaymentStatus,
    symbol: '—',
    title: '',
    description: '',
    action: '',
  },
  methods: {
    onLoad(options: Record<string, string | undefined>) {
      const status = isPaymentStatus(options.status) ? options.status : 'unavailable'
      this.setData({ status, ...CONTENT[status] })
    },
    primaryAction() {
      if (this.data.status === 'pending') {
        wx.showToast({ title: '支付查单接口尚未接入', icon: 'none' })
        return
      }
      const pages = getCurrentPages()
      if (pages.length > 1) {
        wx.navigateBack()
        return
      }
      wx.redirectTo({ url: '/pages/membership/membership' })
    },
    goHome() {
      wx.redirectTo({ url: '/pages/home/home' })
    },
  },
})

function isPaymentStatus(value: string | undefined): value is PaymentStatus {
  return value === 'success' || value === 'pending' || value === 'failed' || value === 'unavailable'
}
