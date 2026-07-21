/// <reference path="./types/index.d.ts" />

declare namespace WechatMiniprogram {
  interface Wx {
    /** 基础库 2.20.1 起支持，替代已废弃的 getSystemInfoSync */
    getWindowInfo?(): { statusBarHeight: number; windowWidth: number; windowHeight: number }
  }
}

interface IAppOption {
  globalData: {
    userInfo?: WechatMiniprogram.UserInfo,
    role: 'provider' | 'client',
    onboardingStatus: 'draft' | 'pending' | 'approved' | 'rejected',
    apiReady: boolean,
  }
  userInfoReadyCallback?: WechatMiniprogram.GetUserInfoSuccessCallback,
}