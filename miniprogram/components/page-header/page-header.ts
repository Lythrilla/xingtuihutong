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
        navigationHeight: menuHeight + gap * 2,
        rightInset,
      })
    },
  },
})
