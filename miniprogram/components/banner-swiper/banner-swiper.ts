import { getBanners, goTo, type Banner } from '../../utils/api'

export {}

Component({
  data: {
    banners: [] as Banner[],
  },
  lifetimes: {
    attached() {
      this.loadBanners()
    },
  },
  methods: {
    async loadBanners() {
      try {
        const banners = await getBanners()
        this.setData({ banners })
      } catch (error) {
        // Banner is non-critical; fail silently.
      }
    },
    onTap(event: WechatMiniprogram.TouchEvent) {
      const link = event.currentTarget.dataset.link as string | undefined
      if (!link) return
      goTo(link)
    },
  },
})
