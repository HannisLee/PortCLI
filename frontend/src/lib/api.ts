// 检测运行模式：Wails 桌面 or 浏览器 WebUI
// 运行时动态检测，避免模块加载时 Wails runtime 尚未注入的问题
function checkIsWails(): boolean {
  return typeof window !== 'undefined' && !!(window as any)['go']?.['main']?.['App']
}

export interface ForwardRule {
  id: string
  sourceHost: string
  localPort: number
  targetHost: string
  targetPort: number
  enabled: boolean
  createdAt: string
}

export interface LogEntry {
  timestamp: string
  source: string
  bytesIn: number
  bytesOut: number
  status: string
}

export interface WebUIConfig {
  enabled: boolean
  port: number
  password: string
}

// ============ Wails 模式 ============
async function wailsGetRules(): Promise<ForwardRule[]> {
  const { GetRules } = await import('../../wailsjs/go/main/App')
  return GetRules()
}
async function wailsAddRule(sourceHost: string, localPort: number, targetHost: string, targetPort: number): Promise<void> {
  const { AddRule } = await import('../../wailsjs/go/main/App')
  return AddRule(sourceHost, localPort, targetHost, targetPort)
}
async function wailsDeleteRule(id: string): Promise<void> {
  const { DeleteRule } = await import('../../wailsjs/go/main/App')
  return DeleteRule(id)
}
async function wailsToggleRule(id: string, enabled: boolean): Promise<void> {
  const { ToggleRule } = await import('../../wailsjs/go/main/App')
  return ToggleRule(id, enabled)
}
async function wailsGetLogs(ruleId: string, limit: number): Promise<LogEntry[]> {
  const { GetLogs } = await import('../../wailsjs/go/main/App')
  return GetLogs(ruleId, limit)
}
async function wailsClearLogs(ruleId: string): Promise<void> {
  const { ClearLogs } = await import('../../wailsjs/go/main/App')
  return ClearLogs(ruleId)
}
async function wailsGetStatus(): Promise<{ [key: string]: boolean }> {
  const { GetStatus } = await import('../../wailsjs/go/main/App')
  return GetStatus()
}
async function wailsGetWebUIConfig(): Promise<WebUIConfig> {
  const { GetWebUIConfig } = await import('../../wailsjs/go/main/App')
  return GetWebUIConfig()
}
async function wailsUpdateWebUIConfig(enabled: boolean, port: number, password: string): Promise<void> {
  const { UpdateWebUIConfig } = await import('../../wailsjs/go/main/App')
  return UpdateWebUIConfig(enabled, port, password)
}

// ============ HTTP 模式 ============
async function httpRequest<T>(path: string, opts?: RequestInit): Promise<T> {
  const res = await fetch(path, {
    headers: { 'Content-Type': 'application/json', ...opts?.headers },
    ...opts,
  })
  if (res.status === 401) {
    throw new Error('UNAUTHORIZED')
  }
  if (!res.ok) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error || res.statusText)
  }
  return res.json()
}

function httpGetRules(): Promise<ForwardRule[]> {
  return httpRequest<ForwardRule[]>('/api/rules')
}
function httpAddRule(sourceHost: string, localPort: number, targetHost: string, targetPort: number): Promise<void> {
  return httpRequest<void>('/api/rules', {
    method: 'POST',
    body: JSON.stringify({ sourceHost, localPort, targetHost, targetPort }),
  })
}
function httpDeleteRule(id: string): Promise<void> {
  return httpRequest<void>(`/api/rules/${id}`, { method: 'DELETE' })
}
function httpToggleRule(id: string, enabled: boolean): Promise<void> {
  return httpRequest<void>(`/api/rules/${id}/toggle`, {
    method: 'PUT',
    body: JSON.stringify({ enabled }),
  })
}
function httpGetLogs(ruleId: string, limit: number): Promise<LogEntry[]> {
  return httpRequest<LogEntry[]>(`/api/rules/${ruleId}/logs?limit=${limit}`)
}
function httpClearLogs(ruleId: string): Promise<void> {
  return httpRequest<void>(`/api/rules/${ruleId}/logs`, { method: 'DELETE' })
}
function httpGetStatus(): Promise<{ [key: string]: boolean }> {
  return httpRequest<{ [key: string]: boolean }>('/api/status')
}
function httpLogin(password: string): Promise<void> {
  return httpRequest<void>('/api/login', {
    method: 'POST',
    body: JSON.stringify({ password }),
  })
}
function httpLogout(): Promise<void> {
  return httpRequest<void>('/api/logout', { method: 'POST' })
}
function httpGetWebUIConfig(): Promise<WebUIConfig> {
  return httpRequest<WebUIConfig>('/api/webui-config')
}
function httpUpdateWebUIConfig(cfg: Partial<WebUIConfig>): Promise<void> {
  return httpRequest<void>('/api/webui-config', {
    method: 'PUT',
    body: JSON.stringify(cfg),
  })
}

// ============ 统一导出 ============
// 每次调用时动态检测运行模式，避免初始化时序问题
function wailsWrap<T>(fn: () => Promise<T>): () => Promise<T> {
  return () => checkIsWails() ? fn() : fn()
}

export const api = {
  getRules: () => checkIsWails() ? wailsGetRules() : httpGetRules(),
  addRule: (sourceHost: string, localPort: number, targetHost: string, targetPort: number) =>
    checkIsWails() ? wailsAddRule(sourceHost, localPort, targetHost, targetPort) : httpAddRule(sourceHost, localPort, targetHost, targetPort),
  deleteRule: (id: string) => checkIsWails() ? wailsDeleteRule(id) : httpDeleteRule(id),
  toggleRule: (id: string, enabled: boolean) => checkIsWails() ? wailsToggleRule(id, enabled) : httpToggleRule(id, enabled),
  getLogs: (ruleId: string, limit: number) => checkIsWails() ? wailsGetLogs(ruleId, limit) : httpGetLogs(ruleId, limit),
  clearLogs: (ruleId: string) => checkIsWails() ? wailsClearLogs(ruleId) : httpClearLogs(ruleId),
  getStatus: () => checkIsWails() ? wailsGetStatus() : httpGetStatus(),
  getWebUIConfig: () => checkIsWails() ? wailsGetWebUIConfig() : httpGetWebUIConfig(),
  updateWebUIConfig: (enabled: boolean, port: number, password: string) =>
    checkIsWails() ? wailsUpdateWebUIConfig(enabled, port, password) : httpUpdateWebUIConfig({ enabled, port, password }),
  login: (password: string) => checkIsWails() ? Promise.resolve() : httpLogin(password),
  logout: () => checkIsWails() ? Promise.resolve() : httpLogout(),
  get isWailsMode() { return checkIsWails() },
}
