export {}

Component({
  data: {
    notices: [
      {
        icon: '✦',
        iconClass: 'mint',
        title: 'AI 匹配成功',
        desc: '为《晴朗以后》找到 3 位高匹配推广伙伴',
        time: '刚刚',
      },
      {
        icon: '￥',
        iconClass: 'gold',
        title: '结算到账',
        desc: '《微醺日落》推广合作款 ¥6,800 已入账',
        time: '2小时前',
      },
    ],
    chats: [
      {
        avatar: '鲸',
        avatarClass: 'aqua',
        name: '鲸浪短视频矩阵',
        message: '方案已更新，新增两位音乐类达人，可以看一下~',
        time: '10:24',
        unread: 2,
      },
      {
        avatar: '鹿',
        avatarClass: 'gold',
        name: '原创新音乐厂牌',
        message: '好的，我们内部确认后今天给您答复',
        time: '昨天',
        unread: 0,
      },
      {
        avatar: '沐',
        avatarClass: 'violet',
        name: '沐光音乐工作室',
        message: '[语音] 关于海外宣发的时间安排',
        time: '昨天',
        unread: 1,
      },
      {
        avatar: '声',
        avatarClass: 'blue',
        name: '品牌活动音乐授权',
        message: '合同已盖章回传，请查收',
        time: '周三',
        unread: 0,
      },
    ],
  },
  methods: {
    openChat(event: WechatMiniprogram.TouchEvent) {
      const name = event.currentTarget.dataset.name as string
      wx.showToast({ title: `会话「${name}」即将开放`, icon: 'none' })
    },
  },
})
