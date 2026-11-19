# Covered Call Options Advisor

Initial full-stack scaffold for an application that will analyze covered call options for a user-selected stock, recommend contracts based on configurable preferences, and send daily email notifications.

## Prerequisites

- Rust and Cargo
- Node.js and npm

## Setup

From the repository root:

```bash
cd backend
cargo build

cd ../frontend
npm install
```

## Run locally

Start the backend:

```bash
cd backend
cargo run
```

The API listens at `http://127.0.0.1:3001`.

In a second terminal, start the frontend:

```bash
cd frontend
npm run dev
```

Open the Vite URL printed in the terminal, normally `http://localhost:5173`.

## Backend configuration

- `BACKEND_ADDRESS` defaults to `127.0.0.1:3001`.
- `DATABASE_URL` defaults to `sqlite:data/covered-call-advisor.db?mode=rwc`.
- The backend reads a local `.env` file when present.
- `SMTP_SERVER` is the SMTP server hostname.
- `SMTP_PORT` defaults to `587`.
- `SMTP_USERNAME` is the SMTP account username.
- `SMTP_PASSWORD` is the SMTP account password.
- `SMTP_FROM` is the sender email address.

Keep SMTP credentials in environment variables or a local `.env` file. Never commit credentials.

## Scheduled email notifications

The backend checks at 10:00 AM America/Toronto each day. It sends at most one notification for each trading date, skips weekends and US exchange holidays, and uses the saved notification email address.

Mock-data runs are always marked as simulated in the email.

To trigger a manual development test:

```bash
curl -X POST http://127.0.0.1:3001/api/notifications/test
```

The backend process must remain running or be deployed to a server for scheduled notifications to work.

## Settings and active position API

- `GET /api/settings`
- `PUT|POST /api/settings`
- `GET /api/position`
- `PUT|POST /api/position`
- `DELETE /api/position`

## Simulated market data API

- `GET /api/market-data/AAPL`
- `GET /api/market-data/MSFT`
- `GET /api/market-data/NVDA`
- `GET /api/market-data/TSLA`

This endpoint uses clearly labeled mock/simulated data only. It does not return live
market quotes or constitute trading advice. The provider is behind a market-data
interface and can later be replaced with a real provider.

## Health check

- API: `http://127.0.0.1:3001/api/health`
- Frontend development proxy: `http://localhost:5173/api/health`
