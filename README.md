# Walrus 🦭

A lightweight CLI time tracking tool written in Rust.

## Installation
```bash
git clone https://github.com/Nifalu/walrus.git
cd walrus
cargo build --release
```

To test it locally:
```bash
./target/release/walrus
```

To install it in your PATH:
```bash
cargo install --path .
```

This installs walrus to `~/.cargo/bin/`, which is automatically in your PATH on most systems.

## Usage

### Basic Commands
```bash
# Start tracking a session
walrus start
walrus start [topic]

# Stop tracking
walrus stop
walrus stop [topic]  # stops a specific topic if multiple sessions are active

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

### Concurrent Sessions

You can track multiple sessions with different topics simultaneously:

```bash
walrus start work       # Start a "work" session
walrus start personal   # Start a "personal" session (in another terminal/tmux)

walrus list            # Shows both as ACTIVE

walrus stop work       # Stops only the "work" session, "personal" continues
walrus stop           # Shows list of active sessions if multiple exist
```

When you try to stop without specifying a topic and multiple sessions are active, walrus will list them and ask you to specify which one to stop.

### Data Location

Database is stored at:
- macOS: `~/Library/Application Support/walrus/walrus.db`
- Linux: `~/.local/share/walrus/walrus.db`
