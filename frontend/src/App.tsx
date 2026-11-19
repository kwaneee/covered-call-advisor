import { useState } from 'react'
import { Dashboard } from './components/Dashboard'
import { Recommendations } from './components/Recommendations'
import './App.css'

function App() {
  const [page, setPage] = useState<'dashboard' | 'recommendations'>('dashboard')

  return (
    <div className="app-shell">
      <header className="topbar">
        <button className="brand" type="button" onClick={() => setPage('dashboard')} aria-label="Go to dashboard">
          <span className="brand-mark">CC</span>
          <span>Covered Call Advisor</span>
        </button>
        <nav className="nav-tabs" aria-label="Primary navigation">
          <button className={page === 'dashboard' ? 'nav-tab nav-tab--active' : 'nav-tab'} type="button" onClick={() => setPage('dashboard')}>Dashboard</button>
          <button className={page === 'recommendations' ? 'nav-tab nav-tab--active' : 'nav-tab'} type="button" onClick={() => setPage('recommendations')}>Recommendations</button>
        </nav>
      </header>
      <main className="main-content">
        {page === 'dashboard' ? <Dashboard /> : <Recommendations />}
      </main>
    </div>
  )
}

export default App
