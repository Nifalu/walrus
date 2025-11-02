# Walrus 🦭

A lightweight CLI time tracking tool written in Rust.

## Installation
```bash
git clone https://github.com/Nifalu/walrus.git
cd walrus
cargo build --release
sudo cp target/release/walrus /usr/local/bin/
```

Or add to your PATH:
```bash
export PATH="$PATH:/path/to/walrus/target/release"
```

## Usage

### Basic Commands
```bash
# Start tracking
walrus start
walrus start [project name]

# Stop tracking
walrus stop

# Show recent sessions
walrus show
walrus show -n 10

# Show periods
walrus show -p day          # today
walrus show -p week         # this week
walrus show -p month        # this month
walrus show -p year         # this year

# Multiple periods
walrus show -p week -n 4    # last 4 weeks
walrus show -p month -n 6   # last 6 months
```

### Managing Sessions
```bash
# List sessions with IDs
walrus list
walrus list -n 20

# Add a session manually
walrus add "topic" -s "31.10.2025 09:00" -e "31.10.2025 12:30"

# Edit a session
walrus edit <id> -t "new topic"
walrus edit <id> -s "31.10.2025 10:00"

# Delete a session
walrus delete <id>

# Export to CSV
walrus export

# Clear all data
walrus reset
```

### Data Location

Database is stored at:
- macOS: `~/Library/Application Support/walrus/walrus.db`
- Linux: `~/.local/share/walrus/walrus.db`
