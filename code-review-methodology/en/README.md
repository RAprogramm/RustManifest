# Code Analysis and Vulnerability Detection Methodology

A comprehensive guide for code review, vulnerability detection, and identifying code weaknesses.

## Methodology Structure

### 1. [Security and Vulnerabilities](./security-vulnerabilities.md)
- Cryptography and authentication
- Injections and data validation
- Replay, CSRF, XSS attacks
- Data and secret leaks
- Practical examples

### 2. [Performance](./performance-issues.md)
- Finding bottlenecks
- Inefficient allocations
- Duplicate operations
- Algorithm optimization
- Profiling

### 3. [Code Quality](./code-quality.md)
- Code duplication (DRY)
- Readability and maintainability
- Error handling
- Testing
- Documentation

### 4. [Architecture](./architecture.md)
- Single Responsibility Principle
- Modularity
- Dependencies
- Reusability
- Scalability

### 5. [Rust Specifics](./rust-specific.md)
- Memory safety
- Ownership and borrowing
- Panic vs Result
- Zero-cost abstractions
- Unsafe code

### 6. [Real Examples](./examples.md)
- Project examples
- Before/After refactoring
- Measurable improvements

## General Code Review Approach

### Stage 1: Initial Overview (5-10 minutes)
1. Understand the overall structure of changes
2. Read commit message and PR description
3. Assess the scope of changes

### Stage 2: Security (15-20 minutes)
1. Vulnerability search (see security-vulnerabilities.md)
2. Input data validation check
3. Cryptography and authentication analysis
4. Secret leak check

### Stage 3: Correctness (10-15 minutes)
1. Does the logic work correctly?
2. All edge cases handled
3. Proper error handling
4. Tests cover functionality

### Stage 4: Performance (10 minutes)
1. Finding inefficient operations
2. Unnecessary allocations
3. Duplicate work
4. Algorithmic complexity

### Stage 5: Quality (10 minutes)
1. Readability
2. Duplication
3. Naming
4. Documentation

### Stage 6: Architecture (5-10 minutes)
1. SOLID principles compliance
2. Modularity
3. Reusability
4. Tech debt

## Quick Checklist

### Critical Issues (block merge)
- [ ] Security vulnerabilities
- [ ] Secret leaks (passwords, tokens, keys)
- [ ] Panic in production code
- [ ] Improper error handling
- [ ] Missing tests for critical logic

### Important Issues (require fixing)
- [ ] Inefficient operations (O(n^2) instead of O(n))
- [ ] Code duplication (>3 repetitions)
- [ ] Missing input data validation
- [ ] Magic numbers and magic strings
- [ ] Undocumented public API

### Desired Improvements
- [ ] Refactoring for readability
- [ ] Additional tests
- [ ] Documentation improvement
- [ ] Performance optimization

## Tools

### Automated Checks
- **Clippy** - static analyzer for Rust
- **cargo audit** - dependency vulnerability check
- **cargo deny** - license and security check
- **cargo tarpaulin** - test coverage
- **cargo bench** - performance benchmarks

### Manual Analysis
- **git diff** - viewing changes
- **ripgrep (rg)** - code pattern search
- **tokei** - code statistics
- **cargo tree** - dependency tree

## Prioritization

### High Priority
1. Security
2. Correctness
3. Critical bugs

### Medium Priority
4. Performance (if issues exist)
5. Code quality
6. Testing

### Low Priority
7. Code style (if autoformatter exists)
8. Minor refactoring
9. Documentation (if not public API)

## Effective Review Principles

1. **Be specific**: "SQL injection possible on line 42" instead of "Security issues"
2. **Suggest solutions**: Not just "This is slow", but "Can use HashMap instead of Vec::find"
3. **Explain why**: "This will lead to replay attack because..."
4. **Use metrics**: "This will increase allocations by 40%"
5. **Be positive**: Note good solutions too

## Next Steps

Study each section in detail:
1. Start with [security-vulnerabilities.md](./security-vulnerabilities.md) - this is most important
2. Then [performance-issues.md](./performance-issues.md) - for production-ready code
3. Study [rust-specific.md](./rust-specific.md) - language specifics
4. Practice on [examples.md](./examples.md) - real cases
