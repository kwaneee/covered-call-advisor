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

## Settings and active position API

- `GET /api/settings`
- `PUT|POST /api/settings`
- `GET /api/position`
- `PUT|POST /api/position`
- `DELETE /api/position`

## Health check

- API: `http://127.0.0.1:3001/api/health`
- Frontend development proxy: `http://localhost:5173/api/health`
