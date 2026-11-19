export interface UserSettings {
  stock_ticker: string
  shares_owned: number
  min_preferred_dte: number
  max_preferred_dte: number
  min_preferred_delta: number
  max_preferred_delta: number
  min_acceptable_premium_per_share: number
  cost_basis_per_share: number | null
  email_address: string
}

export interface ActivePosition {
  stock_ticker: string
  strike_price: number
  expiration_date: string
  contracts: number
  premium_received_per_share: number
  cost_basis_per_share: number | null
}

export interface ActivePositionMonitor {
  position: ActivePosition
  dte: number
  shares_covered: number
  potential_profit_if_called_away: number | null
}

export interface StockInformation {
  ticker: string
  current_price: number
  quote_timestamp: string
  data_source: string
  disclaimer: string
}

export interface CoveredCallRecommendation {
  strike: number
  expiration: string
  dte: number
  delta: number
  bid: number
  ask: number
  midpoint_premium: number
  gross_premium_per_contract: number
  potential_profit_if_called_away: number | null
  contracts_recommended: number
  contract_multiplier: number
  explanation: string
}

export interface RecommendationResponse {
  status: 'RECOMMENDATIONS' | 'WAIT' | 'ACTIVE_POSITION'
  recommendations: CoveredCallRecommendation[]
  active_position: ActivePositionMonitor | null
  wait_reason: string | null
}

export interface MarketData {
  stock: StockInformation
  call_options: unknown[]
}

export class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...init?.headers,
    },
  })

  if (!response.ok) {
    let message = `Request failed with status ${response.status}`
    try {
      const body = (await response.json()) as { error?: string }
      if (body.error) message = body.error
    } catch {
      // Keep the status-based error when the response has no JSON body.
    }
    throw new ApiError(response.status, message)
  }

  if (response.status === 204) return undefined as T
  return response.json() as Promise<T>
}

export const api = {
  getSettings: () => request<UserSettings>('/api/settings'),
  saveSettings: (settings: UserSettings) =>
    request<UserSettings>('/api/settings', {
      method: 'PUT',
      body: JSON.stringify(settings),
    }),
  getPosition: () => request<ActivePosition>('/api/position'),
  getMarketData: (ticker: string) =>
    request<MarketData>(`/api/market-data/${encodeURIComponent(ticker)}`),
  getRecommendations: () => request<RecommendationResponse>('/api/recommendations'),
}
