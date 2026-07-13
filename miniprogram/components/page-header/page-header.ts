export {}

Component({
  properties: {
    title: {
      type: String,
      value: '',
    },
    subtitle: {
      type: String,
      value: '',
    },
    back: {
      type: Boolean,
      value: false,
    },
  },
  data: {
    statusBarHeight: 20,
    navigationHeight: 44,
    rightInset: 96,
  },
  lifetimes: {
    attached() {
      const menu = wx.getMenuButtonBoundingClientRect()
      const systemInfo = wx.getSystemInfoSync()
      const statusBarHeight = systemInfo.statusBarHeight || 20
      const gap = Math.max(menu.top - statusBarHeight, 4)
      const menuHeight = menu.height || 32
      const rightInset =
        menu.left > systemInfo.windowWidth / 2 ? systemInfo.windowWidth - menu.left + 10 : 12
      this.setData({
        statusBarHeight,
        navigationHeight: Math.max(menuHeight + gap * 2, 52),
        rightInset,
      })
    },
  },
  methods: {
    goBack() {
      const pages = getCurrentPages()
      if (pages.length > 1) {
        wx.navigateBack()
        return
      }
      wx.redirectTo({ url: '/pages/home/home' })
    },
  },
})
