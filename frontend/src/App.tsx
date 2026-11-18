import { useEffect, useState } from 'react'
import './App.css'

function App() {
  const [healthStatus, setHealthStatus] = useState('Checking...')

  useEffect(() => {
    fetch('/api/health')
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Health check failed with status ${response.status}`)
        }
        return response.json()
      })
      .then((data: { status: string }) => setHealthStatus(data.status))
      .catch(() => setHealthStatus('Unavailable'))
  }, [])

  return (
    <main>
      <h1>Covered Call Options Advisor</h1>
      <p>Backend status: <strong>{healthStatus}</strong></p>
    </main>
  )
}

export default App
