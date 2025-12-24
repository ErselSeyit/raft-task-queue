# Distributed Task Queue with Raft Consensus

A high-performance, distributed task queue system built in Rust, featuring Raft consensus algorithm for fault tolerance and strong consistency.

## About

This project implements a complete distributed task queue system combining Rust's safety guarantees with the Raft consensus algorithm. It provides a production-ready foundation for building scalable, fault-tolerant task processing systems.

**Key Features:**
- Full Raft implementation with leader election and log replication
- Modern Rust stack with Tokio async runtime and Axum HTTP framework
- Real-time web dashboard for monitoring and task management
- RESTful API for task submission and management
- Concurrent task execution with configurable limits
- Automatic retry logic with configurable max attempts

## Quick Start

### Prerequisites

- Rust 1.70+ ([install from rustup.rs](https://rustup.rs/))

### Running

Start the API server:
```bash
cd api-server
cargo run --release
```

The API server will start on `http://localhost:3000`

Start the dashboard (optional):
```bash
cd dashboard
cargo run --release
```

The dashboard will be available at `http://localhost:8080`

## API Endpoints

- `POST /api/tasks` - Create a new task
- `GET /api/tasks` - List all tasks (optional `?status=Pending` filter)
- `GET /api/tasks/{id}` - Get task by ID
- `DELETE /api/tasks/{id}` - Cancel a task
- `GET /api/stats` - Get task statistics
- `GET /health` - Health check

## Example Usage

Create a task:
```bash
curl -X POST http://localhost:3000/api/tasks \
  -H "Content-Type: application/json" \
  -d '{"name": "compute", "payload": "data", "max_retries": 3}'
```

List tasks:
```bash
curl http://localhost:3000/api/tasks
```

## Architecture

The project is organized as a Cargo workspace:
- `shared` - Common types and message definitions
- `raft-core` - Raft consensus algorithm implementation
- `task-queue` - Task queue and executor logic
- `api-server` - REST API server using Axum
- `dashboard` - Web dashboard for monitoring

## License

MIT License
