# Contributing to Burd

Thank you for your interest in contributing to Burd! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- **macOS** (currently the only supported platform)
- **Node.js** 18+ and npm
- **Rust** (latest stable)
- **Xcode Command Line Tools**

### Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/YOUR_USERNAME/burd.git
   cd burd
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Run in development mode:
   ```bash
   npm run tauri dev
   ```

### Project Structure

```
burd/
├── src/                    # SvelteKit frontend
│   ├── lib/               # Shared components and utilities
│   │   └── sections/      # Main UI sections
│   └── routes/            # Page routes
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── cli/          # CLI command implementations
│   │   ├── commands/     # Tauri command handlers
│   │   ├── services/     # Service definitions
│   │   └── ...
│   └── helper/           # Privileged helper binary
├── docs/                  # Documentation
└── scripts/               # Build and utility scripts
```

## How to Contribute

### Reporting Bugs

1. Check if the issue already exists in the issue tracker
2. If not, create a new issue with:
   - Clear, descriptive title
   - Steps to reproduce
   - Expected vs actual behavior
   - System information (macOS version, chip architecture)
   - Relevant logs or screenshots

### Suggesting Features

1. Open an issue with the "enhancement" label
2. Describe the feature and its use case
3. Discuss the implementation approach if you have ideas

### Submitting Code

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Ensure code passes checks:
   ```bash
   # Rust
   cd src-tauri
   cargo fmt
   cargo clippy
   cargo test

   # Frontend
   npm run check
   ```
5. Commit with clear messages
6. Push and open a Pull Request

### Code Style

#### Rust
- Follow standard Rust conventions
- Use `cargo fmt` for formatting
- Address all `clippy` warnings
- Add doc comments for public APIs

#### Svelte/TypeScript
- Use TypeScript for type safety
- Follow existing component patterns
- Keep components focused and composable

### Commit Messages

Use clear, descriptive commit messages:
- `feat: Add database export functionality`
- `fix: Resolve port conflict detection`
- `docs: Update CLI documentation`
- `refactor: Extract common validation logic`

## Development Tips

### Running the CLI

```bash
cd src-tauri
cargo run --bin burd -- <command>
```

### Building for Release

```bash
npm run tauri build
```

### Debugging

- Rust logs appear in the terminal
- Frontend logs appear in the WebView DevTools (right-click > Inspect)

## Questions?

Feel free to open an issue for any questions about contributing.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
