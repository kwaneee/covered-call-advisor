interface StatusStateProps {
  title: string
  message: string
  tone?: 'error' | 'loading' | 'empty'
}

export function StatusState({ title, message, tone = 'empty' }: StatusStateProps) {
  return (
    <section className={`status-state status-state--${tone}`} role={tone === 'error' ? 'alert' : 'status'}>
      <h2>{title}</h2>
      <p>{message}</p>
    </section>
  )
}
