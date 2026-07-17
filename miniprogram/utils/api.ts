import { API_BASE_URL } from '../config'

export function toAssetUrl(path: string): string {
  if (!path) return path
  if (/^https?:\/\//i.test(path)) return path
  if (path.startsWith('/')) return `${API_BASE_URL}${path}`
  return path
}

type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE'

interface ApiErrorBody {
  error?: string
}

export interface SessionUser {
  id: string
  displayName: string
  organization: string
  role: 'provider' | 'client'
  avatar: string
  description: string
  verified: boolean
  onboardingStatus: 'draft' | 'pending' | 'approved' | 'rejected'
  reviewNote: string
}

interface SessionResponse {
  token: string
  user: SessionUser
}

let sessionPromise: Promise<SessionResponse> | null = null

export async function ensureSession(
  role: 'provider' | 'client' = 'provider',
): Promise<SessionResponse> {
  if (sessionPromise) {
    return sessionPromise
  }
  const token = wx.getStorageSync('starconnect-token') as string
  sessionPromise = rawRequest<SessionResponse>(
    '/api/auth/session',
    'POST',
    { role },
    token || undefined,
  )
    .then((session) => {
      wx.setStorageSync('starconnect-token', session.token)
      wx.setStorageSync('starconnect-role', session.user.role)
      return session
    })
    .finally(() => {
      sessionPromise = null
    })
  return sessionPromise
}

export async function apiRequest<T>(
  path: string,
  method: HttpMethod = 'GET',
  data?: object,
): Promise<T> {
  const role = (wx.getStorageSync('starconnect-role') || 'provider') as 'provider' | 'client'
  const session = await ensureSession(role)
  try {
    return await rawRequest<T>(path, method, data, session.token)
  } catch (error) {
    if (error instanceof ApiRequestError && error.statusCode === 401) {
      wx.removeStorageSync('starconnect-token')
      const renewed = await ensureSession(role)
      return rawRequest<T>(path, method, data, renewed.token)
    }
    throw error
  }
}

export interface WorkUploadResponse {
  url: string
  fileName: string
}

export async function uploadWorkFile(filePath: string): Promise<WorkUploadResponse> {
  const role = (wx.getStorageSync('starconnect-role') || 'provider') as 'provider' | 'client'
  const session = await ensureSession(role)
  return new Promise((resolve, reject) => {
    wx.uploadFile({
      url: `${API_BASE_URL}/api/uploads/work`,
      filePath,
      name: 'file',
      timeout: 60000,
      header: {
        Authorization: `Bearer ${session.token}`,
      },
      success(response) {
        let body: WorkUploadResponse & ApiErrorBody
        try {
          body = JSON.parse(response.data) as WorkUploadResponse & ApiErrorBody
        } catch {
          reject(new ApiRequestError('上传响应异常，请重试', response.statusCode))
          return
        }
        if (response.statusCode >= 200 && response.statusCode < 300) {
          resolve({ ...body, url: toAssetUrl(body.url) })
          return
        }
        reject(
          new ApiRequestError(
            friendlyError(body.error) || `上传失败（${response.statusCode}）`,
            response.statusCode,
          ),
        )
      },
      fail() {
        reject(new ApiRequestError('作品上传失败，请检查网络后重试', 0))
      },
    })
  })
}

export async function uploadAvatarFile(filePath: string): Promise<WorkUploadResponse> {
  const role = (wx.getStorageSync('starconnect-role') || 'provider') as 'provider' | 'client'
  const session = await ensureSession(role)
  return new Promise((resolve, reject) => {
    wx.uploadFile({
      url: `${API_BASE_URL}/api/uploads/avatar`,
      filePath,
      name: 'file',
      timeout: 30000,
      header: {
        Authorization: `Bearer ${session.token}`,
      },
      success(response) {
        let body: WorkUploadResponse & ApiErrorBody
        try {
          body = JSON.parse(response.data) as WorkUploadResponse & ApiErrorBody
        } catch {
          reject(new ApiRequestError('上传响应异常，请重试', response.statusCode))
          return
        }
        if (response.statusCode >= 200 && response.statusCode < 300) {
          resolve({ ...body, url: toAssetUrl(body.url) })
          return
        }
        reject(
          new ApiRequestError(
            friendlyError(body.error) || `上传失败（${response.statusCode}）`,
            response.statusCode,
          ),
        )
      },
      fail() {
        reject(new ApiRequestError('头像上传失败，请检查网络后重试', 0))
      },
    })
  })
}

export interface MembershipPlan {
  key: string
  name: string
  price: number
  description: string
}

export interface MembershipPlansResponse {
  plans: MembershipPlan[]
  activeUntil?: string
  balance: number
}

export async function getMembershipPlans(): Promise<MembershipPlansResponse> {
  return apiRequest<MembershipPlansResponse>('/api/membership/plans')
}

export async function purchaseMembership(plan: string): Promise<{ success: boolean; activeUntil?: string; balance: number }> {
  return apiRequest<{ success: boolean; activeUntil?: string; balance: number }>('/api/membership/purchase', 'POST', { plan })
}

export async function unlockPartnerContact(partnerId: string): Promise<{ contactMethod: string; unlocked: boolean; balance: number }> {
  return apiRequest<{ contactMethod: string; unlocked: boolean; balance: number }>(`/api/partners/${encodeURIComponent(partnerId)}/unlock`, 'POST')
}

class ApiRequestError extends Error {
  constructor(
    message: string,
    readonly statusCode: number,
  ) {
    super(message)
  }
}

function rawRequest<T>(
  path: string,
  method: HttpMethod,
  data?: object,
  token?: string,
): Promise<T> {
  return new Promise((resolve, reject) => {
    wx.request({
      url: `${API_BASE_URL}${path}`,
      method,
      data,
      timeout: 12000,
      header: {
        'content-type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      success(response) {
        if (response.statusCode >= 200 && response.statusCode < 300) {
          resolve(response.data as unknown as T)
          return
        }
        const body = response.data as unknown as ApiErrorBody
        reject(
          new ApiRequestError(
            friendlyError(body.error) || `请求失败（${response.statusCode}）`,
            response.statusCode,
          ),
        )
      },
      fail() {
        reject(new ApiRequestError('网络连接异常，请检查后重试', 0))
      },
    })
  })
}

function friendlyError(message?: string): string {
  if (!message) return ''
  const messages: Record<string, string> = {
    'amount must be positive': '请输入有效金额',
    'insufficient wallet balance': '可提现余额不足',
    'invalid song or budget': '歌曲或预算选项已失效，请重新选择',
    'message content is required': '消息内容不能为空',
    'organization is required': '机构或个人名称不能为空',
    'conversation not found': '该会话不存在或已失效',
    'partner not found': '该合作伙伴已下架',
    'plan not found': '该推广方案已下架',
    'submitted role cannot be changed': '已提交入驻申请，暂不能切换身份',
    'onboarding required fields are missing': '请完整填写必填资料',
    'creator work information is required': '请上传一份代表作品',
    'creator verification checklist is required': '请确认全部作品声明',
    'unsupported work file type': '请上传音频、视频或图片作品',
    'work file size is invalid': '作品文件需小于 30MB',
    'at least one specialty is required': '请至少选择一个能力或风格标签',
    'onboarding approval required': '入驻审核通过后才能发起合作',
    'creator role required': '仅创作者可以发布推广匹配',
    'no approved providers available': '暂无已审核推广方，请稍后再试',
    'creator wallet is not available': '创作者身份暂无钱包功能',
    'agent message is too long': '消息过长，请精简后重试',
    'unsupported avatar image type': '请上传 jpg/png/webp 格式头像',
    'avatar image must be <= 2MB': '头像需小于 2MB',
    'invalid membership plan': '会员套餐选择错误',
    'contact not available': '该主页暂未配置可解锁联系方式',
    'cannot unlock same role contact': '不能解锁同身份联系方式',
    'cannot unlock own contact': '不能解锁自己的联系方式',
  }
  return messages[message] || message
}
