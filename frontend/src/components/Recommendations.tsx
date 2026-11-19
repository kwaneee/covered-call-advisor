import { useEffect, useState } from 'react'
import { api, type RecommendationResponse } from '../api'
import { StatusState } from './StatusState'

export function Recommendations() {
  const [response, setResponse] = useState<RecommendationResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')

  useEffect(() => {
    let active = true
    api.getRecommendations()
      .then((data) => { if (active) setResponse(data) })
      .catch((requestError) => {
        if (active) setError(requestError instanceof Error ? requestError.message : 'Unable to load recommendations')
      })
      .finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [])

  if (loading) return <StatusState title="Loading recommendations" message="Ranking qualifying contracts." tone="loading" />
  if (error) return <StatusState title="Recommendations unavailable" message={error} tone="error" />
  if (!response) return <StatusState title="No analysis" message="No recommendation data is available." />

  if (response.status === 'ACTIVE_POSITION' && response.active_position) {
    const monitor = response.active_position
    const position = monitor.position
    return (
      <section className="card active-position-card">
        <div className="card-heading">
          <div>
            <p className="eyebrow">Active position</p>
            <h2>{position.stock_ticker} covered call</h2>
          </div>
          <span className="status-pill status-pill--active-position">Monitoring</span>
        </div>
        <div className="active-position-grid">
          <Metric label="Strike" value={currency(position.strike_price)} />
          <Metric label="Expiration" value={position.expiration_date} />
          <Metric label="DTE" value={String(monitor.dte)} />
          <Metric label="Contracts" value={String(position.contracts)} />
          <Metric label="Shares covered" value={number(monitor.shares_covered)} />
          <Metric label="Premium / share" value={currency(position.premium_received_per_share)} />
          <Metric label="Cost basis" value={position.cost_basis_per_share == null ? 'Not set' : currency(position.cost_basis_per_share)} />
          <Metric label="Potential profit if called" value={monitor.potential_profit_if_called_away == null ? 'Unavailable' : currency(monitor.potential_profit_if_called_away)} />
        </div>
        <p className="position-note">{response.wait_reason}</p>
      </section>
    )
  }

  if (response.status === 'WAIT') {
    return (
      <section className="wait-card">
        <span className="wait-symbol" aria-hidden="true">!</span>
        <p className="eyebrow">Recommendation status</p>
        <h2>WAIT</h2>
        <p>{response.wait_reason || 'No call options qualify under the current preferences.'}</p>
      </section>
    )
  }

  return (
    <section>
      <div className="recommendations-heading">
        <div>
          <p className="eyebrow">Ranked contracts</p>
          <h2>Covered call recommendations</h2>
        </div>
        <span className="simulated-badge">Simulated data</span>
      </div>
      <div className="recommendation-list">
        {response.recommendations.map((option, index) => (
          <article className="recommendation-card" key={`${option.expiration}-${option.strike}`}>
            <div className="rank">#{index + 1}</div>
            <div className="contract-main">
              <p className="eyebrow">{option.expiration}</p>
              <h3>{currency(option.strike)} strike</h3>
              <p>{option.dte} DTE · Delta {option.delta.toFixed(2)}</p>
            </div>
            <dl className="contract-values">
              <div><dt>Bid</dt><dd>{currency(option.bid)}</dd></div>
              <div><dt>Ask</dt><dd>{currency(option.ask)}</dd></div>
              <div><dt>Midpoint</dt><dd>{currency(option.midpoint_premium)} / share</dd></div>
              <div><dt>Gross premium</dt><dd>{currency(option.gross_premium_per_contract)}</dd></div>
              <div><dt>Potential profit</dt><dd>{option.potential_profit_if_called_away == null ? 'Unavailable' : currency(option.potential_profit_if_called_away)}</dd></div>
              <div><dt>Contracts</dt><dd>{option.contracts_recommended} × {option.contract_multiplier}</dd></div>
            </dl>
            <p className="qualification"><strong>Why it qualifies</strong>{option.explanation}</p>
          </article>
        ))}
      </div>
    </section>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div className="metric"><span>{label}</span><strong>{value}</strong></div>
}

function currency(value: number) {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value)
}

function number(value: number) {
  return new Intl.NumberFormat('en-US').format(value)
}
