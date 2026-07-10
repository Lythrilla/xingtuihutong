/// <reference path="./types/index.d.ts" />

interface IAppOption {
  globalData: {
    userInfo?: WechatMiniprogram.UserInfo,
    role: 'provider' | 'client',
  }
  userInfoReadyCallback?: WechatMiniprogram.GetUserInfoSuccessCallback,
}