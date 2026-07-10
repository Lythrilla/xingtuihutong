export {}

interface PlazaEntry {
  type: string
  avatar: string
  avatarClass: string
  name: string
  identity: string
  description: string
  tags: string[]
  match: number
  result: string
}

Component({
  data: {
    activeType: 'all',
    types: [
      { key: 'all', label: '全部' },
      { key: 'provider', label: '服务方' },
      { key: 'client', label: '被服务方' },
      { key: 'latest', label: '最新' },
    ],
    entries: [
      {
        type: 'provider',
        avatar: '鲸',
        avatarClass: 'aqua',
        name: '鲸浪短视频矩阵',
        identity: '认证服务方',
        description: '擅长流行、说唱新歌冷启动，覆盖 300+ 优质音乐账号',
        tags: ['短视频推广', '内容策划', '数据复盘'],
        match: 98,
        result: '合作案例 23',
      },
      {
        type: 'client',
        avatar: '鹿',
        avatarClass: 'gold',
        name: '原创新音乐厂牌',
        identity: '被服务方',
        description: '寻找校园与达人渠道，推广青春流行单曲《晴朗以后》',
        tags: ['流行音乐', '校园渠道', '预算充足'],
        match: 95,
        result: '预算 ¥10,000',
      },
      {
        type: 'provider',
        avatar: '声',
        avatarClass: 'blue',
        name: '品牌活动音乐授权',
        identity: '认证服务方',
        description: '商业品牌音乐营销、线下活动授权与创意整合服务',
        tags: ['品牌营销', '版权授权', '线下活动'],
        match: 92,
        result: '合作案例 15',
      },
      {
        type: 'client',
        avatar: '沐',
        avatarClass: 'violet',
        name: '沐光音乐工作室',
        identity: '被服务方',
        description: '独立电子音乐人团队，寻找高审美视觉与海外宣发伙伴',
        tags: ['电子音乐', '视觉设计', '海外宣发'],
        match: 89,
        result: '预算 ¥26,000',
      },
    ],
    visibleEntries: [] as PlazaEntry[],
  },
  lifetimes: {
    attached() {
      this.setData({ visibleEntries: this.data.entries })
    },
  },
  methods: {
    changeType(event: WechatMiniprogram.TouchEvent) {
      const key = event.currentTarget.dataset.key as string
      const visibleEntries =
        key === 'all' || key === 'latest'
          ? this.data.entries
          : this.data.entries.filter((entry) => entry.type === key)
      this.setData({ activeType: key, visibleEntries })
    },
    connect(event: WechatMiniprogram.TouchEvent) {
      const name = event.currentTarget.dataset.name as string
      wx.showToast({ title: `已向${name}发起沟通`, icon: 'none' })
    },
  },
})
