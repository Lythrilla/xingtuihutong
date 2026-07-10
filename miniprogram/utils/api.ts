import { API_BASE_URL } from '../config'

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
  }
  return messages[message] || message
}
