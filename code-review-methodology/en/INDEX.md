# Code Review Methodology Navigation

## All Materials

### Quick Start
**[quick-reference.md](./quick-reference.md)** - Daily cheat sheet
- 5-minute checklist
- Quick problem-finding commands
- Common error patterns
- **Start here!**

---

### Main Sections

#### 1. **[README.md](./README.md)** - Methodology Overview
- General approach to code review
- Documentation structure
- Problem prioritization
- Effective review principles

#### 2. **[security-vulnerabilities.md](./security-vulnerabilities.md)** - Security
- Replay attacks
- SQL/Command injections
- Secret leaks
- Cryptography issues
- **16 KB - MOST IMPORTANT**

#### 3. **[performance-issues.md](./performance-issues.md)** - Performance
- Inefficient allocations
- Duplicate operations
- Algorithmic complexity
- Blocking operations
- **15 KB**

#### 4. **[code-quality.md](./code-quality.md)** - Code Quality
- DRY principle
- Readability
- Documentation
- Testing
- Architectural principles
- **16 KB**

#### 5. **[rust-specific.md](./rust-specific.md)** - Rust Specifics
- Ownership and Borrowing
- Lifetimes
- Panic vs Result
- Unsafe code
- Traits and Generics
- **14 KB**

#### 6. **[examples.md](./examples.md)** - Real Examples
- Telegram Authentication analysis
- N+1 Query Problem
- Memory Leaks
- Before/After refactoring with metrics
- **17 KB**

---

## Usage Scenarios

### Scenario 1: "Need to quickly review a PR"
1. Open **[quick-reference.md](./quick-reference.md)**
2. Use the 5-15 minute checklist
3. Run problem-finding commands

### Scenario 2: "Found a vulnerability, need details"
1. Open **[security-vulnerabilities.md](./security-vulnerabilities.md)**
2. Find the vulnerability type
3. Check examples and solutions

### Scenario 3: "Code is slow, where to look"
1. Open **[performance-issues.md](./performance-issues.md)**
2. Check the section by category (allocations/algorithms/copying)
3. Use profiling tools

### Scenario 4: "Need to improve architecture"
1. Open **[code-quality.md](./code-quality.md)**
2. Section "Architectural principles"
3. Check SRP, modularity, DI

### Scenario 5: "Rust-specific problem"
1. Open **[rust-specific.md](./rust-specific.md)**
2. Find the category (ownership/lifetimes/unsafe)
3. Check correct code examples

### Scenario 6: "Want to learn from examples"
1. Open **[examples.md](./examples.md)**
2. Read the Telegram Authentication example
3. Check improvement metrics

---

## File Sizes

```
code-quality.md            16 KB
examples.md                17 KB
performance-issues.md      15 KB
quick-reference.md         7.7 KB
README.md                  6.5 KB
rust-specific.md           14 KB
security-vulnerabilities.md 16 KB
```

**Total: ~92 KB** of detailed methodology

---

## Code Review Process

```
+---------------------------------------------+
|  1. Open PR and read description            |
|     Time: 2-3 minutes                       |
+------------------+--------------------------+
                   |
                   v
+---------------------------------------------+
|  2. SECURITY (security-vulnerabilities.md)  |
|     - Secrets, injections, validation       |
|     Time: 5-10 minutes                      |
+------------------+--------------------------+
                   |
                   v
+---------------------------------------------+
|  3. PERFORMANCE (performance-issues.md)     |
|     - Duplication, O(n^2), allocations      |
|     Time: 5-10 minutes                      |
+------------------+--------------------------+
                   |
                   v
+---------------------------------------------+
|  4. QUALITY (code-quality.md)               |
|     - DRY, readability, tests               |
|     Time: 5-10 minutes                      |
+------------------+--------------------------+
                   |
                   v
+---------------------------------------------+
|  5. RUST-SPECIFIC (rust-specific.md)        |
|     - Ownership, unsafe, lifetimes          |
|     Time: 5 minutes                         |
+------------------+--------------------------+
                   |
                   v
+---------------------------------------------+
|  6. Write comments                          |
|     - Specific, with solution examples      |
|     Time: 5-10 minutes                      |
+------------------+--------------------------+
                   |
                   v
              Approve / Request Changes
```

**Total time: 30-45 minutes** for a thorough review

---

## Tips

1. **Use search**: All files are markdown, easy to search with Ctrl+F
2. **Start with quick-reference**: For daily use
3. **Return to examples**: examples.md contains real cases
4. **Copy commands**: All bash commands are ready to use
5. **Take notes**: Add your own findings to the methodology

---

## How to Read

### New to code review:
1. README.md - understand the general approach
2. quick-reference.md - memorize basic checks
3. examples.md - learn from examples
4. The rest - as needed

### Experienced reviewer:
1. quick-reference.md - use as a checklist
2. Specific sections - when you need details
3. examples.md - for metrics reference

---

## Updating the Methodology

When you find new problem types:
1. Add to the corresponding file
2. Update quick-reference.md if it's a common problem
3. Add to examples.md if you have a good example

---

## File Structure

```
code-review-methodology/
|-- en/                           # English version
|   |-- INDEX.md                  # This file - navigation
|   |-- README.md                 # General overview
|   |-- quick-reference.md        # Cheat sheet
|   |-- security-vulnerabilities.md # Security
|   |-- performance-issues.md     # Performance
|   |-- code-quality.md           # Code quality
|   |-- rust-specific.md          # Rust specifics
|   +-- examples.md               # Real examples
+-- ru/                           # Russian version
    +-- ...
```

---

**Created**: 2025-10-26
**Version**: 1.0
**Author**: Based on practical code review experience
