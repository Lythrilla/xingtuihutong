export {}

const app = getApp<IAppOption>()

Component({
  data: {
    roleLabel: '服务方',
    metrics: [
      { value: '12', label: '进行中项目' },
      { value: '8', label: '待处理需求' },
      { value: '23,450', label: '本月收益/元' },
    ],
    recommendations: [
      {
        avatar: '昱',
        avatarClass: 'aqua',
        title: '短视频矩阵推广',
        subtitle: '适合新歌冷启动 · 预计覆盖 80w+',
        score: 98,
        price: '¥18,000',
      },
      {
        avatar: '许',
        avatarClass: 'violet',
        title: '达人种草计划',
        subtitle: '许棠音乐工作室 · 粉丝 12.3w',
        score: 95,
        price: '¥9,800',
      },
      {
        avatar: '声',
        avatarClass: 'blue',
        title: '校园音乐人联盟',
        subtitle: '覆盖 24 所高校 · 年轻乐迷',
        score: 93,
        price: '¥6,500',
      },
    ],
  },
  lifetimes: {
    attached() {
      const isProvider = app.globalData.role === 'provider'
      this.setData({
        roleLabel: isProvider ? '服务方' : '被服务方',
        metrics: isProvider
          ? this.data.metrics
          : [
              { value: '7', label: '推广中歌曲' },
              { value: '16', label: '合作服务方' },
              { value: '128w', label: '本月曝光' },
            ],
      })
    },
  },
  methods: {
    openAI() {
      wx.redirectTo({ url: '/pages/ai/ai' })
    },
    openMatch() {
      wx.redirectTo({ url: '/pages/match/match' })
    },
    openPlaza() {
      wx.redirectTo({ url: '/pages/plaza/plaza' })
    },
  },
})
