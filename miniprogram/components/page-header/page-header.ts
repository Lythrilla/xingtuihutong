import { getWindowMetrics } from '../../utils/api'

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
      const windowInfo = getWindowMetrics()
      const statusBarHeight = windowInfo.statusBarHeight || 20
      const gap = Math.max(menu.top - statusBarHeight, 4)
      const menuHeight = menu.height || 32
      const rightInset =
        menu.left > windowInfo.windowWidth / 2 ? windowInfo.windowWidth - menu.left + 10 : 12
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
      wx.switchTab({ url: '/pages/home/home' })
    },
  },
})
