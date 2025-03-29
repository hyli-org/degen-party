# Degen Party 🎮

A delightful mini-game collection built with Vue.js. This project offers a colorful, engaging gaming experience with various mini-games and interactive features.

## 🍄 Features

- **Playful Interface:** Vibrant colors, engaging animations, and a festive atmosphere
- **Mini-Games:** Simple and fun games with engaging mechanics

## 🏃‍♂️ Getting Started

### Prerequisites

- Node.js (v14+)
- Bun

### Installation

1. Clone the repository:

```bash
git clone https://github.com/yourusername/degen-party.git
cd degen-party
```

2. Install dependencies:

```bash
bun install
```

3. Start the development server:

```bash
bun run dev
```

4. Open your browser and navigate to `http://localhost:3000`

## Docker Build Instructions

### Quick Development Build

```bash
docker build -t rust-helper2 -f Dockerfile.rust .

# Build frontend
bun run build

# Build Docker image (uses debug build by default)
docker build -t degen-party-mac .
```

### Production Build for linux

```bash
docker build -t rust-helper -f Dockerfile.rust --platform linux/amd64 .

# Build frontend
bun run build

# Build Docker image with release binary
docker build -t degen-party  --platform linux/amd64 .
docker tag degen-party europe-west3-docker.pkg.dev/hyle-413414/hyle-docker/degen-party:latest
docker push europe-west3-docker.pkg.dev/hyle-413414/hyle-docker/degen-party:latest
```

Run the container:

```bash
docker run -p 80:80 degen-party
```

The application will be available at:

- Frontend: http://localhost
- WebSocket endpoint: ws://localhost/ws
