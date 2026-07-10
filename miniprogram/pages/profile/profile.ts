export {}

const app = getApp<IAppOption>()

Component({
  data: {
    roleLabel: '服务方',
    stats: [
      { value: '96%', label: '交付好评率' },
      { value: '38', label: '完成合作' },
      { value: '4.9', label: '综合评分' },
    ],
    tags: ['短视频推广', '内容策划', '达人矩阵', '音乐营销', '数据复盘'],
  },
  lifetimes: {
    attached() {
      if (app.globalData.role === 'client') {
        this.setData({
          roleLabel: '被服务方',
          stats: [
            { value: '28', label: '入驻歌曲' },
            { value: '16', label: '合作伙伴' },
            { value: '128w', label: '累计曝光' },
          ],
          tags: ['流行音乐', '电子音乐', '版权清晰', '长期合作', '品牌授权'],
        })
      }
    },
  },
  methods: {
    changeRole() {
      wx.redirectTo({ url: '/pages/index/index' })
    },
    edit() {
      wx.showToast({ title: '资料编辑功能即将开放', icon: 'none' })
    },
  },
})
