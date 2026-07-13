export {}

Component({
  data: {
    selectedPlan: 'monthly',
    purchasing: false,
    plans: [
      {
        key: 'monthly',
        name: '月度会员',
        description: '适合持续寻找合作方',
        price: '待定价',
        badge: '推荐',
      },
      {
        key: 'quarterly',
        name: '季度会员',
        description: '额度按周期发放，规则清晰',
        price: '待定价',
        badge: '',
      },
      {
        key: 'single',
        name: '按次解锁',
        description: '没有会员也可单次购买',
        price: '新人优惠待配置',
        badge: '新人',
      },
    ],
  },
  methods: {
    selectPlan(event: WechatMiniprogram.TouchEvent) {
      this.setData({ selectedPlan: event.currentTarget.dataset.key as string })
    },
    purchase() {
      if (this.data.purchasing) return
      this.setData({ purchasing: true })
      wx.showModal({
        title: '当前不会扣款',
        content: '套餐价格、权益账本与微信支付仍在接入。本页面先确认购买前信息结构，当前不会创建订单或扣除费用。',
        confirmText: '查看结果页',
        cancelText: '暂不购买',
        success: (result) => {
          if (result.confirm) {
            wx.navigateTo({ url: '/pages/payment-result/payment-result?status=unavailable' })
          }
        },
        complete: () => this.setData({ purchasing: false }),
      })
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
