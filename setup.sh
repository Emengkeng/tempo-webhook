#!/bin/bash

set -e

echo "🚀 Tempo Webhook Service - Setup Script"
echo "========================================="
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust found: $(rustc --version)"

# Check if PostgreSQL is accessible
if [ -z "$DATABASE_URL" ]; then
    echo "⚠️  DATABASE_URL not set. Using default: postgres://postgres:postgres@localhost/tempo_webhooks"
    export DATABASE_URL="postgres://postgres:postgres@localhost/tempo_webhooks"
fi

# Check if .env exists
if [ ! -f .env ]; then
    echo "📝 Creating .env file from template..."
    cp .env.example .env
    
    # Generate secrets
    echo "🔐 Generating secrets..."
    JWT_SECRET=$(openssl rand -hex 32)
    API_KEY_SECRET=$(openssl rand -hex 32)
    
    # Update .env file
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/your_jwt_secret_here_replace_with_random_32_bytes/$JWT_SECRET/" .env
        sed -i '' "s/your_encryption_key_here_replace_with_random_32_bytes/$API_KEY_SECRET/" .env
    else
        sed -i "s/your_jwt_secret_here_replace_with_random_32_bytes/$JWT_SECRET/" .env
        sed -i "s/your_encryption_key_here_replace_with_random_32_bytes/$API_KEY_SECRET/" .env
    fi
    
    echo "✅ .env file created with generated secrets"
    echo ""
    echo "⚠️  IMPORTANT: Please edit .env and set the following:"
    echo "   - DATABASE_URL (if different from default)"
    echo "   - POLAR_SECRET_KEY (from Polar dashboard)"
    echo "   - POLAR_WEBHOOK_SECRET (from Polar dashboard)"
    echo ""
else
    echo "✅ .env file already exists"
fi

# Install sqlx-cli if not present
if ! command -v sqlx &> /dev/null; then
    echo "📦 Installing sqlx-cli..."
    cargo install sqlx-cli --no-default-features --features postgres
else
    echo "✅ sqlx-cli found"
fi

# Load environment variables
source .env 2>/dev/null || true

# Create database if it doesn't exist
echo "🗄️  Setting up database..."
if command -v psql &> /dev/null; then
    DB_NAME=$(echo $DATABASE_URL | sed 's/.*\///')
    psql -lqt | cut -d \| -f 1 | grep -qw $DB_NAME || {
        echo "Creating database: $DB_NAME"
        createdb $DB_NAME
    }
    echo "✅ Database ready"
else
    echo "⚠️  psql not found. Please ensure PostgreSQL is installed and database exists."
fi

# Run migrations
echo "🔄 Running database migrations..."
sqlx migrate run

echo "✅ Migrations completed"
echo ""

# Build the project
echo "🔨 Building project..."
cargo build --release

echo ""
echo "✅ Setup complete!"
echo ""
echo "Next steps:"
echo "1. Start Redis: docker run -d -p 6379:6379 redis:7"
echo "2. Start NATS: docker run -d -p 4222:4222 nats:latest"
echo "3. Review and update .env file with your settings"
echo "4. Run the service: cargo run"
echo ""
echo "For production deployment:"
echo "  flyctl deploy"
echo ""
echo "📚 Documentation: See README.md for full documentation"
