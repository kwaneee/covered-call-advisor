# Copilot Project Instructions

## Project

Covered Call Options Advisor is a full-stack application for analyzing covered call options. The initial stage is a working backend/frontend shell only.

## Technology

- Backend: Rust
- Backend framework: Axum
- Async runtime: Tokio
- Frontend: React with TypeScript
- Frontend tooling: Vite
- Database: SQLite
- Notifications: SMTP email
- Development: VS Code

## Architecture

- `backend/` contains the Rust API. Axum routes, application state, database access, and notification logic should remain server-side.
- `frontend/` contains the React user interface. Keep API calls in small modules as the application grows.
- Vite proxies `/api` requests to the Axum server during local development.
- SQLite should be used only after the initial feature stage; do not add database code until requested.

## Coding conventions

- Prefer small, clear functions and explicit types.
- Format Rust with `cargo fmt`; format frontend code consistently.
- Keep dependencies minimal and avoid adding packages when standard APIs are sufficient.
- Use async Rust with Tokio and Axum conventions.
- Use React function components and TypeScript.
- Keep API responses typed and use clear HTTP status codes.
- Add tests beside the relevant code and run the smallest applicable test suite.

## Planned features

1. Select one stock ticker for analysis.
2. Retrieve market and options data.
3. Configure covered call preferences such as expiration window, strike selection, and minimum return.
4. Score and recommend suitable covered call contracts.
5. Store user preferences and analysis history in SQLite.
6. Send configurable daily email notifications through SMTP.

## Current scope

Do not implement financial calculations, market-data integrations, SQLite persistence, or notification delivery unless a later request explicitly starts that stage.
