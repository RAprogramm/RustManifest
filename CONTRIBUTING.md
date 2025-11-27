# Contributing to <PROJECT_NAME>

[Русская версия](CONTRIBUTING.ru.md)

Thank you for your interest in contributing to this project!

## Code Style & Standards

This project follows the [RustManifest](https://github.com/RAprogramm/RustManifest) coding standards. Please read it thoroughly before contributing.

Key points:
- Use `cargo +nightly fmt` for formatting
- No `unwrap()` or `expect()` in production code
- Documentation via Rustdoc only (no inline comments)
- Descriptive naming conventions

## Development Setup

### Prerequisites

- Rust nightly toolchain
- cargo-make (optional, for task automation)
- cargo-nextest (for running tests)

### Installation

```bash
git clone https://github.com/RAprogramm/<PROJECT_NAME>
cd <PROJECT_NAME>

# Install nightly toolchain
rustup toolchain install nightly
rustup component add rustfmt --toolchain nightly
rustup component add clippy

# Install test runner (optional but recommended)
cargo install cargo-nextest
```

### Pre-commit Checks

Before committing, ensure all checks pass:

```bash
# Format check
cargo +nightly fmt --all -- --check

# Linting
cargo clippy --all-targets --all-features -- -D warnings

# Tests
cargo test --all-features

# Or with nextest
cargo nextest run --all-features
```

## Git Workflow

### Branch Naming

Use issue number as branch name:
```
123
```

### Commit Messages

Format: `#<issue_number> <type>: <description>`

```
#123 feat: add new output format
#123 fix: correct line counting in parser
#45 docs: update API examples
#78 test: add property tests for extractor
#90 refactor: simplify config loading
```

Types:
- `feat` - new feature
- `fix` - bug fix
- `docs` - documentation
- `test` - tests
- `refactor` - code refactoring
- `chore` - maintenance tasks

### Pull Requests

1. Create branch from `main`
2. Make your changes
3. Ensure all CI checks pass
4. Create PR with descriptive title
5. Include `Closes #<issue>` in description

### Keep Production Code Changes Small

**Code review quality degrades with size.** What matters is *production code*, not total lines changed.

| Production Code | Review Quality | Risk |
|-----------------|----------------|------|
| < 100 lines | Thorough | Low |
| 100-300 lines | Moderate | Medium |
| 300+ lines | Superficial | High |

**Tests and benchmarks don't count the same way:**

- A PR with 50 lines in `src/` and 1000 lines of tests is **easy to review**
- A PR with 300 lines in `src/` and 0 tests is **hard to review and risky**

Use [rust-prod-diff-checker](https://github.com/RAprogramm/rust-prod-diff-checker) GitHub Action to automatically analyze PR size and separate production changes from tests/benchmarks.

**Rule of thumb:** If a reviewer can't understand your production code changes in 15 minutes, the PR is too big.

## Testing

### Writing Tests

- Cover all public API functions
- Test error paths, not just happy paths
- Use property-based testing for parsers
- No `unwrap()` in tests - use `?` with proper error types

```rust
#[test]
fn test_example() -> Result<(), Box<dyn std::error::Error>> {
    let result = some_function()?;
    assert_eq!(result, expected);
    Ok(())
}
```

### Running Tests

```bash
# All tests
cargo test --all-features

# With coverage
cargo llvm-cov nextest --all-features

# Specific test
cargo test test_name
```

## CI/CD Pipeline

Every PR triggers:

| Job | Description |
|-----|-------------|
| Format | `cargo +nightly fmt --check` |
| Clippy | `cargo clippy -D warnings` |
| Test | `cargo test --all-features` |
| Doc | `cargo doc --no-deps` |
| Coverage | Upload to Codecov |

## Documentation

All public items must have Rustdoc:

```rust
/// Short description of what this function does.
///
/// # Errors
///
/// Returns `Error` if something fails.
///
/// # Examples
///
/// ```
/// use my_crate::my_function;
///
/// let result = my_function()?;
/// # Ok::<(), my_crate::Error>(())
/// ```
pub fn my_function() -> Result<(), Error> {
    // ...
}
```

## Getting Help

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
- Provide minimal reproduction for bugs

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
