# Database Setup Script
Write-Host "🔧 Smart Memo Database Setup" -ForegroundColor Green

Write-Host "`nChoose your database option:" -ForegroundColor Yellow
Write-Host "1. PostgreSQL (Docker) - Recommended for production" -ForegroundColor Cyan
Write-Host "2. SQLite - Quick testing" -ForegroundColor Cyan
Write-Host "3. PostgreSQL (Local) - If you have PostgreSQL installed" -ForegroundColor Cyan

$choice = Read-Host "`nEnter your choice (1-3)"

switch ($choice) {
    "1" {
        Write-Host "`n🐘 Setting up PostgreSQL with Docker..." -ForegroundColor Yellow
        
        # Check if Docker is running
        try {
            docker ps | Out-Null
            Write-Host "✅ Docker is running" -ForegroundColor Green
        } catch {
            Write-Host "❌ Docker is not running. Please start Docker Desktop first." -ForegroundColor Red
            exit 1
        }
        
        # Start PostgreSQL container
        Write-Host "Starting PostgreSQL container..." -ForegroundColor Yellow
        docker run --name smartmemo-postgres -e POSTGRES_PASSWORD=mark42 -e POSTGRES_DB=memo -p 5432:5432 -d postgres:15
        
        # Wait for PostgreSQL to start
        Write-Host "Waiting for PostgreSQL to start..." -ForegroundColor Yellow
        Start-Sleep -Seconds 15
        
        # Set environment variable
        $env:DATABASE_URL = "postgres://postgres:mark42@localhost:5432/memo"
        Write-Host "✅ PostgreSQL container started" -ForegroundColor Green
    }
    "2" {
        Write-Host "`n🗃️ Setting up SQLite..." -ForegroundColor Yellow
        $env:DATABASE_URL = "sqlite://./memo.db?mode=rwc"
        Write-Host "✅ SQLite configuration set" -ForegroundColor Green
    }
    "3" {
        Write-Host "`n🐘 Using local PostgreSQL..." -ForegroundColor Yellow
        $env:DATABASE_URL = "postgres://postgres:mark42@localhost:5432/memo"
        Write-Host "✅ PostgreSQL configuration set" -ForegroundColor Green
        Write-Host "Make sure PostgreSQL is running locally!" -ForegroundColor Yellow
    }
    default {
        Write-Host "❌ Invalid choice. Exiting." -ForegroundColor Red
        exit 1
    }
}

# Run migrations
Write-Host "`n🔄 Running database migrations..." -ForegroundColor Yellow
cd migration
cargo run up
cd ..

# Test the application
Write-Host "`n🧪 Testing application..." -ForegroundColor Yellow
cargo run

Write-Host "`n✅ Setup complete!" -ForegroundColor Green
