import { useEffect, useState, type FormEvent } from 'react'
import { ApiError, api, type MarketData, type RecommendationResponse, type UserSettings } from '../api'
import { StatusState } from './StatusState'

const DEFAULT_SETTINGS: UserSettings = {
  stock_ticker: 'AAPL',
  shares_owned: 300,
  min_preferred_dte: 21,
  max_preferred_dte: 45,
  min_preferred_delta: 0.20,
  max_preferred_delta: 0.35,
  min_acceptable_premium_per_share: 1.25,
  cost_basis_per_share: null,
  email_address: '',
}

type FormValues = {
  [K in keyof UserSettings]: string
}

export function Dashboard() {
  const [settings, setSettings] = useState<UserSettings | null>(null)
  const [form, setForm] = useState<FormValues | null>(null)
  const [marketData, setMarketData] = useState<MarketData | null>(null)
  const [recommendation, setRecommendation] = useState<RecommendationResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    let active = true

    async function load() {
      setLoading(true)
      setError('')
      try {
        let savedSettings: UserSettings
        let hasSavedSettings = true
        try {
          savedSettings = await api.getSettings()
        } catch (requestError) {
          if (requestError instanceof ApiError && requestError.status === 404) {
            savedSettings = DEFAULT_SETTINGS
            hasSavedSettings = false
          } else {
            throw requestError
          }
        }
        if (!active) return
        setSettings(savedSettings)
        setForm(toForm(savedSettings))

        if (!hasSavedSettings) return

        try {
          const [market, latest] = await Promise.all([
            api.getMarketData(savedSettings.stock_ticker),
            api.getRecommendations(),
          ])
          if (active) {
            setMarketData(market)
            setRecommendation(latest)
          }
        } catch (requestError) {
          if (active) setError(requestError instanceof Error ? requestError.message : 'Unable to load market analysis')
        }
      } catch (requestError) {
        if (active) setError(requestError instanceof Error ? requestError.message : 'Unable to load dashboard')
      } finally {
        if (active) setLoading(false)
      }
    }

    void load()
    return () => { active = false }
  }, [])

  function updateField(field: keyof UserSettings, value: string) {
    setSaved(false)
    setForm((current) => current ? { ...current, [field]: value } : current)
  }

  async function savePreferences(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!form) return

    const next: UserSettings = {
      stock_ticker: form.stock_ticker.trim().toUpperCase(),
      shares_owned: Number(form.shares_owned),
      min_preferred_dte: Number(form.min_preferred_dte),
      max_preferred_dte: Number(form.max_preferred_dte),
      min_preferred_delta: Number(form.min_preferred_delta),
      max_preferred_delta: Number(form.max_preferred_delta),
      min_acceptable_premium_per_share: Number(form.min_acceptable_premium_per_share),
      cost_basis_per_share: form.cost_basis_per_share === '' ? null : Number(form.cost_basis_per_share),
      email_address: form.email_address.trim(),
    }

    setSaving(true)
    setError('')
    setSaved(false)
    try {
      const savedSettings = await api.saveSettings(next)
      setSettings(savedSettings)
      setForm(toForm(savedSettings))
      setSaved(true)
      const [market, latest] = await Promise.all([
        api.getMarketData(savedSettings.stock_ticker),
        api.getRecommendations(),
      ])
      setMarketData(market)
      setRecommendation(latest)
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : 'Unable to save preferences')
    } finally {
      setSaving(false)
    }
  }

  if (loading) return <StatusState title="Loading dashboard" message="Retrieving saved preferences and market data." tone="loading" />
  if (!settings || !form) return <StatusState title="Dashboard unavailable" message={error || 'Preferences could not be loaded.'} tone="error" />

  return (
    <div className="dashboard-grid">
      <section className="card overview-card">
        <div className="card-heading">
          <div>
            <p className="eyebrow">Portfolio overview</p>
            <h2>{settings.stock_ticker}</h2>
          </div>
          <span className="simulated-badge">Simulated data</span>
        </div>
        <div className="metrics-grid">
          <Metric label="Current stock price" value={marketData ? currency(marketData.stock.current_price) : 'Unavailable'} />
          <Metric label="Shares owned" value={number(settings.shares_owned)} />
          <Metric label="Cost basis" value={settings.cost_basis_per_share == null ? 'Not set' : currency(settings.cost_basis_per_share)} />
          <Metric label="Notification email" value={settings.email_address || 'Not set'} />
        </div>
        {marketData && (
          <p className="data-disclaimer">{marketData.stock.disclaimer}</p>
        )}
      </section>

      <section className="card recommendation-summary">
        <p className="eyebrow">Latest analysis</p>
        {recommendation ? (
          <>
            <div className={`status-pill status-pill--${recommendation.status.toLowerCase().replace('_', '-')}`}>
              {recommendation.status.replace('_', ' ')}
            </div>
            {recommendation.status === 'ACTIVE_POSITION' && recommendation.active_position ? (
              <div>
                <h3>Active covered call</h3>
                <p><strong>${recommendation.active_position.position.strike_price.toFixed(2)}</strong> strike expires {recommendation.active_position.position.expiration_date}</p>
                <p>{recommendation.active_position.dte} DTE · {recommendation.active_position.shares_covered} shares covered</p>
              </div>
            ) : recommendation.status === 'RECOMMENDATIONS' && recommendation.recommendations[0] ? (
              <div>
                <h3>Top recommendation</h3>
                <p><strong>${recommendation.recommendations[0].strike.toFixed(2)}</strong> strike · {recommendation.recommendations[0].dte} DTE</p>
                <p>{currency(recommendation.recommendations[0].midpoint_premium)} midpoint / share</p>
              </div>
            ) : (
              <div>
                <h3>WAIT</h3>
                <p>{recommendation.wait_reason}</p>
              </div>
            )}
          </>
        ) : (
          <p>{error || 'Analysis unavailable.'}</p>
        )}
      </section>

      <section className="card preferences-card">
        <div className="card-heading">
          <div>
            <p className="eyebrow">Saved preferences</p>
            <h2>Investment criteria</h2>
          </div>
        </div>
        <form className="preferences-form" onSubmit={savePreferences}>
          <label>Stock ticker<input value={form.stock_ticker} onChange={(event) => updateField('stock_ticker', event.target.value)} required /></label>
          <label>Shares owned<input type="number" min="0" step="1" value={form.shares_owned} onChange={(event) => updateField('shares_owned', event.target.value)} required /></label>
          <label>Minimum DTE<input type="number" min="0" step="1" value={form.min_preferred_dte} onChange={(event) => updateField('min_preferred_dte', event.target.value)} required /></label>
          <label>Maximum DTE<input type="number" min="0" step="1" value={form.max_preferred_dte} onChange={(event) => updateField('max_preferred_dte', event.target.value)} required /></label>
          <label>Minimum Delta<input type="number" min="0" max="1" step="0.01" value={form.min_preferred_delta} onChange={(event) => updateField('min_preferred_delta', event.target.value)} required /></label>
          <label>Maximum Delta<input type="number" min="0" max="1" step="0.01" value={form.max_preferred_delta} onChange={(event) => updateField('max_preferred_delta', event.target.value)} required /></label>
          <label>Minimum premium / share<input type="number" min="0" step="0.01" value={form.min_acceptable_premium_per_share} onChange={(event) => updateField('min_acceptable_premium_per_share', event.target.value)} required /></label>
          <label>Cost basis / share<input type="number" min="0" step="0.01" value={form.cost_basis_per_share} onChange={(event) => updateField('cost_basis_per_share', event.target.value)} placeholder="Optional" /></label>
          <label className="wide-field">Notification email<input type="email" value={form.email_address} onChange={(event) => updateField('email_address', event.target.value)} required /></label>
          <div className="form-actions wide-field">
            <button className="primary-button" type="submit" disabled={saving}>{saving ? 'Saving...' : 'Save preferences'}</button>
            {saved && <span className="saved-message" role="status">Preferences saved</span>}
          </div>
        </form>
        {error && <p className="inline-error" role="alert">{error}</p>}
      </section>
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div className="metric"><span>{label}</span><strong>{value}</strong></div>
}

function toForm(settings: UserSettings): FormValues {
  return {
    stock_ticker: settings.stock_ticker,
    shares_owned: String(settings.shares_owned),
    min_preferred_dte: String(settings.min_preferred_dte),
    max_preferred_dte: String(settings.max_preferred_dte),
    min_preferred_delta: String(settings.min_preferred_delta),
    max_preferred_delta: String(settings.max_preferred_delta),
    min_acceptable_premium_per_share: String(settings.min_acceptable_premium_per_share),
    cost_basis_per_share: settings.cost_basis_per_share == null ? '' : String(settings.cost_basis_per_share),
    email_address: settings.email_address,
  }
}

function currency(value: number) {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value)
}

function number(value: number) {
  return new Intl.NumberFormat('en-US').format(value)
}
