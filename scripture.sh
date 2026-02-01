#!/bin/bash
# Convenience script for scripture CLI

# Build if needed
if [ ! -f "target/debug/scripture" ]; then
    echo "🔨 Building scripture CLI..."
    cargo build
fi

# Run with all arguments passed through
./target/debug/scripture "$@"