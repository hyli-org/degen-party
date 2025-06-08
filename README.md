# Orange Trail 🍊

Orange Trail is a game inspired by the classic Oregon Trail, but with a twist: it features a betting minigame! Set on Mars, players venture out to find resources to feed their Hyligotchi. Will you survive the harsh Martian landscape and keep your Hyligotchi thriving?

## 🏃‍♂️ Getting Started

### Prerequisites

- Bun

### Installation

1. Clone the repository:

```bash
git clone https://github.com/yourusername/degen-party.git
cd degen-party
```

2. Install front dependencies:

```bash
cd front
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

Run the container:

```bash
docker run -p 80:80 degen-party
```

The application will be available at:

- Frontend: http://localhost
- WebSocket endpoint: ws://localhost/ws
