/// <reference path="./types/index.d.ts" />

interface IAppOption {
  globalData: {
    userInfo?: WechatMiniprogram.UserInfo,
    role: 'provider' | 'client',
    onboardingStatus: 'draft' | 'pending' | 'approved' | 'rejected',
    apiReady: boolean,
  }
  userInfoReadyCallback?: WechatMiniprogram.GetUserInfoSuccessCallback,
}