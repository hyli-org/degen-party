FROM rust-helper as builder
WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy your source code
COPY . .

# Build the project
RUN cd board-game-engine && cargo build --bin rollup

FROM debian:bookworm-slim
WORKDIR /app
# Install Nginx and runtime dependencies
RUN apt-get update && apt-get install -y \
    nginx \
    libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from builder stage
COPY --from=builder /usr/src/app/board-game-engine/target/debug/rollup ./

# Copy frontend dist and other files
COPY dist/ ./dist/
COPY nginx.conf /etc/nginx/nginx.conf

# Create and copy the entrypoint script
COPY docker-entrypoint.sh ./
RUN chmod +x docker-entrypoint.sh

EXPOSE 80 8080
ENTRYPOINT ["./docker-entrypoint.sh"] 