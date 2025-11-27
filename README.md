<h1>Rust Code Style Guide</h1>

<p align="right">
  <strong>English</strong> | <a href="./README.ru.md">Русский</a>
</p>

<h2>1. Formatting</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Format all Rust code using <code>nightly rustfmt</code>.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > 
> > <pre><code>cargo +nightly fmt --</code></pre>
> >
> >
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>Consistent formatting enhances readability, reduces merge conflicts, and makes code reviews smoother. It ensures that every team member’s code adheres to a unified standard.</p>
> > >
> > > <details>
> > > <summary><strong>Examples &amp; Further Explanation</strong></summary>
> > >
> > > > <p>For instance, a well-formatted codebase allows new team members to quickly understand the project structure and logic. Automated formatting saves time and minimizes stylistic debates during code reviews.</p>
> > > </details>
> > 
> > </details>
> 
> </details>

<h3>1.1 .rustfmt.toml Configuration</h3>

> [!TIP]
>
> <p>
>   <strong>Use the following <code>.rustfmt.toml</code> configuration to ensure consistent formatting across the project.</strong>
> </p>
>
> <details>
> <summary><strong>Configuration</strong></summary>
>
> ```toml
> # Do not add trailing commas if there is only one element
> trailing_comma = "Never"
>
> # Keep braces on the same line where possible
> brace_style = "SameLineWhere"
>
> # Align struct fields if their length is below the threshold
> struct_field_align_threshold = 20
>
> # Format comments inside documentation
> wrap_comments = true
> format_code_in_doc_comments = true
>
> # Do not collapse struct literals into a single line
> struct_lit_single_line = false
>
> # Maximum line width
> max_width = 99
>
> # Grouping imports
> imports_granularity = "Crate"          # Group imports by crate
> group_imports = "StdExternalCrate"     # Separate groups: std, external crates, local
> reorder_imports = true                 # Sort imports within groups
>
> # Enable unstable features (nightly only)
> unstable_features = true
> ```
>
> </details>
>
> <details>
> <summary><strong>Why is this important?</strong></summary>
>
> <p>
> This configuration enforces clarity and consistency. It reduces unnecessary diffs in pull requests, makes code reviews easier, and ensures that both style and readability remain predictable across the team.
> </p>
>
> <details>
> <summary><strong>Examples &amp; Further Explanation</strong></summary>
>
> ### Example without configuration
>
> ```rust
> use std::fmt; use std::io; use serde::Serialize;
>
> struct Person {name:String,age:u32}
>
> impl Person{
>     pub fn new(name:String,age:u32)->Self{
>         Self{name,age}
>     }
> }
> ```
>
> ### Example with configuration
>
> ```rust
> use std::{fmt, io};
>
> use serde::Serialize;
>
> struct Person {
>     name: String,
>     age:  u32
> }
>
> impl Person {
>     pub fn new(name: String, age: u32) -> Self {
>         Self {
>             name,
>             age
>         }
>     }
> }
> ```
>
> <p>
> Notice how the imports are grouped and sorted, struct fields are aligned for readability, and braces are consistently placed on the same line. This reduces noise in diffs and makes the codebase approachable for both newcomers and experienced contributors.
> </p>
> </details>
>
> </details>



<h2>2. Naming Conventions</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Use clear, descriptive names that reflect purpose. Follow snake_case for variables/functions, PascalCase for types, and SCREAMING_SNAKE_CASE for constants.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
> 
> > - **Descriptive Names:**
> >   - <code>create_user_handler</code> – OK
> >   - <code>create_user_service</code> – OK
> >   - <code>create</code> – NO
> >   - <code>create_user</code> – NO
> >
> > - Follow Rust's <em>snake_case</em> for variables and functions.
> > - Use <em>PascalCase</em> for structs and enums (e.g., <code>TransactionStatus</code>).
> > - Constants should be in SCREAMING_SNAKE_CASE.
> >
> >
> > <details>
> > <summary><strong>Why Descriptive Naming?</strong></summary>
> > <p>
> >   Descriptive names reduce ambiguity, facilitate easier onboarding, and improve maintainability. Clear names make it evident what a function or variable does, avoiding misunderstandings and conflicts.
> > </p>
> > 
> > > <details>
> > > <summary><strong>Examples &amp; Further Explanation</strong></summary>
> > > <p>
> > >   For example, <code>create_user_handler</code> indicates that the function is responsible for handling user creation in a web context, whereas a generic name like <code>create</code> gives no context.
> > > </p>
> > > </details>
> > 
> > </details>
> 
> </details>


<h2>3. Code Quality</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Write clean, maintainable code. Avoid unnecessary complexity, panics, and cloning. Minimize global state and restrict use of <code>::</code> to import statements. Do not use <code>mod.rs</code> files.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
> 
> > - Write clean and maintainable code.
> > - Avoid unnecessary complexity.
> > - **Avoid unnecessary <code>unwrap()</code> and <code>clone()</code>.**
> > - Minimize global state and side effects.
> > - Use <code>::</code> only in import statements.
> > - **Do not use <code>mod.rs</code> files.**
> >
> >
> > <details>
> > <summary><strong>Examples &amp; Further Explanation</strong></summary>
> > <p>
> >   Instead of writing <code>some_option.unwrap()</code>, prefer:
> > </p>
> > <pre><code>let value = some_option.ok_or("Expected a value, but found None")?;</code></pre>
> > <p>
> >   This propagates errors properly and avoids crashing the application. Similarly, favor organizing modules in separate <code>module_name.rs</code> files rather than using legacy <code>mod.rs</code> files, which simplifies project structure and improves module discoverability.
> > </p>
> > </details>
> 
> </details>

<h2>4. Branch Management</h2>

> [!NOTE]
>
> <p>
>   <strong>Each branch, commit, and PR must correspond directly to a GitHub Issue number. This ensures automatic linking, clean history, and full traceability.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > - **Create a Branch Named Only by Issue Number:**  
> >   Each branch name must be exactly the Issue number.  
> >   Example:  
> >   ```bash
> >   git checkout -b 123
> >   ```
> >
> > - **Use Auto-Linking in Commits:**  
> >   To make GitHub automatically link commits to Issues, always start the commit message with `#` followed by the Issue number and a space.  
> >   Example:  
> >   ```bash
> >   #123 implement login session restore  
> >   #123 fix null pointer in user handler
> >   ```
> >
> > - **Pull Request Title = Branch Name:**  
> >   The PR title must be the same as the branch name (just the Issue number).  
> >   Example:  
> >   ```
> >   123
> >   ```
> >
> > - **Add an Auto-Close Reference:**  
> >   In the PR description, always include:  
> >   ```
> >   Closes #123
> >   ```
> >   This automatically closes the Issue when the PR is merged.
> >
> > - **Clean Up After Merge:**  
> >   Enable “Delete branch on merge” in repository settings, so merged branches are automatically removed.  
> >   The chain Issue → Branch → Commits → PR → Merge remains fully linked.
> >
> > - **Keep the Repository Clean:**  
> >   Every branch must correspond to an active Issue.  
> >   No orphaned or experimental branches should remain after merge.
> >
> >
> > <details>
> > <summary><strong>Real-World Example &amp; Explanation</strong></summary>
> > <p>
> >   Suppose you are assigned Issue <code>#123</code> to fix a login session bug.  
> >   You create a branch named <code>123</code> and start committing with messages like:
> > </p>
> >
> >   <pre>
> >   #123 implement login session restore  
> >   #123 add retry logic for session token refresh  
> >   </pre>
> >
> >   Then you open a PR titled <code>123</code> with the description:
> >
> >   <pre>
> >   Closes #123
> >   </pre>
> >
> >   When the PR is merged, GitHub automatically closes Issue #123, deletes the branch, and shows all related commits in the Issue timeline.
> >   This creates a perfectly traceable and automated workflow with minimal manual steps.
> > </details>
>
> </details>


<h2>5. Best Practices</h2>

> [!TIP]
>
> <p>
>   <strong>Follow best practices to maintain high code quality.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
> 
> > - Use <code>cargo clippy</code> for linting.
> > - Handle errors gracefully using <code>Result</code> and <code>Option</code>.
> > - **Avoid unnecessary panics.**
> >
> >
> > <details>
> > <summary><strong>Examples &amp; Further Explanation</strong></summary>
> > <p>
> >   Instead of writing:
> > </p>
> > <pre><code>let value = some_option.unwrap();</code></pre>
> > <p>
> >   use:
> > </p>
> > <pre><code>let value = some_option.ok_or("Expected a value, but found None")?;</code></pre>
> > <p>
> >   This pattern ensures errors are propagated and handled appropriately, increasing the robustness of your application.
> > </p>
> > </details>
> 
> </details>


<h2>6. Panics in Rust</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Avoid panics in production; use proper error handling with <code>Result</code> and the <code>?</code> operator.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
> 
> > - **Avoid panics in production code.**
> > - **Discouraged:** Avoid using <code>unwrap()</code> and <code>expect()</code> unless absolutely certain that an error cannot occur.
> > - **Preferred:** Use proper error handling with <code>Result</code> and the <code>?</code> operator.
> >
> >
> > <details>
> > <summary><strong>Examples &amp; Further Explanation</strong></summary>
> > <p>
> >   For example, instead of:
> > </p>
> > <pre><code>let config = Config::from_file("config.toml").unwrap();</code></pre>
> > <p>
> >   use:
> > </p>
> > <pre><code>let config = Config::from_file("config.toml")
> >   .map_err(|e| format!("Failed to load config: {}", e))?;
> > </code></pre>
> > <p>
> >   This approach logs detailed error messages and gracefully propagates errors up the call stack, leading to a more robust and maintainable system.
> > </p>
> > </details>
> >
> > <details>
> > <summary><strong>Real-World Incident: Cloudflare Outage (November 2025)</strong></summary>
> > <p>
> >   On November 18, 2025, a single <code>.unwrap()</code> call in Rust code caused a massive outage across Cloudflare's 330+ datacenters. Services like ChatGPT, X, Canva, and many others went offline for approximately 3 hours.
> > </p>
> > <p>
> >   The root cause: a configuration change caused a features file to contain more entries than expected. The Rust code checked a limit but used <code>unwrap()</code> on an error path instead of handling it gracefully. When the limit was exceeded, the code panicked with: <code>"thread fl2_worker_thread panicked: called Result::unwrap() on an Err value"</code>
> > </p>
> > <p>
> >   <strong>Lesson:</strong> The <code>.unwrap()</code> had been in the codebase for a long time but was never triggered until unexpected input reached that code path. This is why production code must handle all error cases explicitly.
> > </p>
> > <p>
> >   <a href="https://blog.cloudflare.com/18-november-2025-outage/">Read the official Cloudflare post-mortem</a>
> > </p>
> > </details>
>
> </details>

<h2>7. Testing & CI</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>All commits must pass pre-commit checks. Tests, formatting, linting, and security scans are enforced both locally (via pre-commit hooks) and remotely (via CI).</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > - **Pre-commit Hooks**
> >   - Installed via:  
> >     <pre><code>cargo make install-hooks</code></pre>
> >   - Automatically run before each commit:  
> >     <pre><code>cargo +nightly fmt -- </code></pre> 
> >     <pre><code> cargo clippy -D warnings  </code></pre>
> >      <pre><code> cargo test --all</code></pre>
> >   - Prevent committing unformatted code, warnings, or failing tests.  
> >
> > - **Unit Tests**
> >   - Cover public functions and error cases.  
> >   - Tests must not rely on <code>unwrap()</code> or <code>expect()</code>.  
> >
> > - **Integration Tests**
> >   - Cover public API, placed in the <code>tests/</code> directory.  
> >
> > - **Doctests**
> >   - All <code>///</code> examples must compile and pass with <code>cargo test --doc</code>.
> >
> > - **Coverage (cargo-llvm-cov + Codecov)**
> >   - Install: <pre><code>cargo install cargo-llvm-cov</code></pre>
> >   - Run locally: <pre><code>cargo llvm-cov --all-features --workspace --html</code></pre>
> >   - CI configuration:
> >     ```yaml
> >     - name: Install cargo-llvm-cov
> >       uses: taiki-e/install-action@cargo-llvm-cov
> >     - name: Generate code coverage
> >       run: cargo llvm-cov --all-features --workspace --codecov --output-path codecov.json
> >     - name: Upload coverage to Codecov
> >       uses: codecov/codecov-action@v5
> >       with:
> >         token: ${{ secrets.CODECOV_TOKEN }}
> >         files: codecov.json
> >         fail_ci_if_error: true
> >     ```
> >
> > <details>
> > <summary><strong>Why cargo-llvm-cov + Codecov?</strong></summary>
> >
> > > - **Precision**: LLVM-based instrumentation provides accurate line and branch coverage, more reliable than source-based tools
> > > - **Speed**: Significantly faster than tarpaulin, especially on large codebases with many dependencies
> > > - **Native format**: Direct codecov.json output without intermediate conversion steps
> > > - **Visualization**: Codecov dashboard shows coverage trends over time, PR coverage diffs, and interactive sunburst charts
> > > - **PR integration**: Automatic coverage reports as PR comments, showing exactly which lines are covered/uncovered
> > > - **Branch protection**: Configure minimum coverage thresholds to fail CI when coverage drops
> > > - **Rust toolchain**: Uses rustc's built-in instrumentation, ensuring compatibility with all Rust features
> > </details>
> >
> >
> > <details>
> > <summary><strong>Examples &amp; Further Explanation</strong></summary>
> >
> > ### Example Unit Test
> >
> > ```rust
> > #[cfg(test)]
> > mod tests {
> >     use super::*;
> >
> >     #[test]
> >     fn test_basic_math() {
> >         assert_eq!(2 + 2, 4);
> >     }
> > }
> > ```
> >
> > ### Example Integration Test
> >
> > ```rust
> > // tests/config_tests.rs
> > use my_crate::load_config;
> >
> > #[test]
> > fn load_valid_config() {
> >     let result = load_config("tests/data/valid.toml");
> >     assert!(result.is_ok());
> > }
> > ```
> >
> > <p>
> >   This workflow enforces correctness at every step: developers cannot commit broken code, and CI ensures nothing slips through at merge time.
> > </p>
> > </details>
>
> </details>

<h2>8. Code Documentation (No Inline Comments)</h2>

> [!NOTE]
>
> <p>
>   <strong>No inline code comments. All explanations live in docblocks attached to modules, structs, enums, traits, functions, and methods.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > - **No line comments in code:**  
> >   Avoid `// ...` and `/* ... */` for explanations of behavior, intent, or invariants. Keep code clean and self-explanatory.
> >
> > - **Use Rust doc comments consistently:**  
> >   - Crate/module: use <code>//!</code> at the top of <code>lib.rs</code> or module files for module-level docs.  
> >   - Items (structs, enums, traits, fns, methods): use <code>///</code> on the item.
> >
> > - **Structure docblocks for IDEs and LSPs:**  
> >   Use headings Rustdoc understands so hovers and Treesitter outlines are stable and rich:
> >   - <code># Overview</code> short purpose
> >   - <code># Examples</code> minimal, compilable samples
> >   - <code># Errors</code> precise failure modes for <code>Result</code>
> >   - <code># Panics</code> only if unavoidable (should be rare)
> >   - <code># Safety</code> if <code>unsafe</code> is used (shouldn’t be)
> >   - <code># Performance</code> if complexity or allocations matter
> >
> > - **Write for other engineers:**  
> >   Be explicit about contracts, inputs, outputs, invariants, and edge cases. Keep examples runnable. Prefer clarity over cleverness.
> >
> > - **Keep docs close to code:**  
> >   Update docblocks with code changes in the same PR. Out-of-date docs are worse than none.
> >
> >
> > <details>
> > <summary><strong>Correct vs Incorrect (Rust)</strong></summary>
> >
> > **Incorrect (inline comments that won’t surface in hovers):**
> >
> > ```rust
> > // Calculates checksum and validates header
> > // Returns Err if invalid
> > pub fn verify(pkt: &Packet) -> Result<(), VerifyError> {
> >     // fast path
> >     if pkt.header.len() < MIN {
> >         return Err(VerifyError::TooShort);
> >     }
> >     // slow path...
> >     Ok(())
> > }
> > ```
> >
> > **Correct (docblocks; IDE hover shows the contract):**
> >
> > ```rust
> > /// # Overview
> > /// Verifies packet header and payload consistency.
> > ///
> > /// # Examples
> > /// ```
> > /// # use mynet::{Packet, verify};
> > /// # fn demo(mut p: Packet) {
> > /// #   // prepare p...
> > /// #   let _ = verify(&p).unwrap();
> > /// # }
> > /// ```
> > ///
> > /// # Errors
> > /// - `VerifyError::TooShort` when header is smaller than the required minimum.
> > /// - `VerifyError::ChecksumMismatch` when computed checksum differs.
> > pub fn verify(pkt: &Packet) -> Result<(), VerifyError> {
> >     if pkt.header.len() < MIN {
> >         return Err(VerifyError::TooShort);
> >     }
> >     // internal micro-notes for maintainers are allowed if they aid refactoring
> >     // (but not to explain business logic). Keep them brief.
> >     Ok(())
> > }
> > ```
> >
> > **Module-level docs instead of a comment banner:**
> >
> > ```rust
> > //! Cryptographic key management and signing primitives.
> > //!
> > //! Provides deterministic ECDSA with explicit domain separation.
> > //!
> > //! # Examples
> > //! ```
> > //! # use keys::{Keypair, Signer};
> > //! # fn demo() {
> > //! #   let kp = Keypair::generate();
> > //! #   let sig = kp.sign(b"payload");
> > //! #   assert!(kp.verify(b"payload", &sig).is_ok());
> > //! # }
> > //! ```
> > pub mod crypto { /* ... */ }
> > ```
> > </details>
> >
> > <details>
> > <summary><strong>Real-World Rationale</strong></summary>
> > <p>
> >   This policy ensures stable IDE/LSP hovers, better Treesitter outlines, and reliable navigation. Engineers see contracts immediately, CI can lint docs, and examples stay compilable. Code remains clean while documentation remains discoverable and accurate.
> > </p>
> > </details>
>
> </details>


<h2>9. Code Review Methodology</h2>

> [!TIP]
>
> <p>
>   <strong>Use the comprehensive code review methodology to find vulnerabilities, performance issues, and quality problems systematically.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > **Available in two languages:**
> > - [English Documentation](./code-review-methodology/en/INDEX.md)
> > - [Russian Documentation](./code-review-methodology/ru/INDEX.md)
> >
> > **Quick Links:**
> >
> > | Topic | EN | RU |
> > |-------|----|----|
> > | Quick Reference | [Cheat Sheet](./code-review-methodology/en/quick-reference.md) | [Шпаргалка](./code-review-methodology/ru/quick-reference.md) |
> > | Security | [Vulnerabilities](./code-review-methodology/en/security-vulnerabilities.md) | [Уязвимости](./code-review-methodology/ru/security-vulnerabilities.md) |
> > | Performance | [Issues](./code-review-methodology/en/performance-issues.md) | [Проблемы](./code-review-methodology/ru/performance-issues.md) |
> > | Code Quality | [Quality](./code-review-methodology/en/code-quality.md) | [Качество](./code-review-methodology/ru/code-quality.md) |
> > | Rust Patterns | [Specifics](./code-review-methodology/en/rust-specific.md) | [Специфика](./code-review-methodology/ru/rust-specific.md) |
> > | Examples | [Real Cases](./code-review-methodology/en/examples.md) | [Примеры](./code-review-methodology/ru/examples.md) |
> >
> >
> > <details>
> > <summary><strong>What's Covered</strong></summary>
> >
> > > **Security Vulnerabilities:**
> > > - Replay attacks and authentication bypasses
> > > - SQL/Command injections
> > > - Secret leaks and cryptography issues
> > > - Input validation problems
> > >
> > > **Performance Issues:**
> > > - Inefficient allocations and unnecessary cloning
> > > - O(n^2) algorithms where O(n) is possible
> > > - Duplicate operations and double parsing
> > > - Blocking operations in async code
> > >
> > > **Code Quality:**
> > > - DRY violations and code duplication
> > > - Naming and readability
> > > - Documentation standards
> > > - Testing coverage
> > >
> > > **Rust-Specific:**
> > > - Ownership and borrowing patterns
> > > - Panic vs Result handling
> > > - Unsafe code review
> > > - Trait bounds and generics
> > </details>
> >
> > <details>
> > <summary><strong>Quick 5-Minute Checklist</strong></summary>
> >
> > **Security (2 min):**
> > - [ ] No secrets in code
> > - [ ] No `unwrap()`/`expect()` in production
> > - [ ] Input data validated
> > - [ ] No SQL/Command injections
> >
> > **Performance (1 min):**
> > - [ ] No obvious O(n^2)
> > - [ ] No duplicate operations
> > - [ ] `Vec::with_capacity()` where needed
> >
> > **Quality (2 min):**
> > - [ ] No code duplication (> 3 times)
> > - [ ] Functions < 50 lines
> > - [ ] Tests for new logic
> > </details>
>
> </details>


<h2>10. Docker & CI Caching for Rust</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Use cargo-chef for Docker layer caching and registry cache for CI. This dramatically reduces build times for unchanged dependencies.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > **The Problem:**
> > - Rust compilation is slow, especially for large dependency trees
> > - Docker rebuilds everything when any file changes
> > - `--mount=type=cache` doesn't persist between CI runners
> > - Each CI run starts from scratch without proper caching
> >
> > **The Solution: cargo-chef + Registry Cache**
> >
> > 1. **cargo-chef** separates dependency compilation from source compilation
> > 2. **Registry cache** persists Docker layers between CI runs
> > 3. Dependencies are cached as a separate layer that only rebuilds when Cargo.toml/Cargo.lock change
> >
> > <details>
> > <summary><strong>Dockerfile Pattern</strong></summary>
> >
> > ```dockerfile
> > # syntax=docker/dockerfile:1
> > ARG RUST_VERSION=1.83.0
> >
> > # Chef stage - install cargo-chef
> > FROM rust:${RUST_VERSION} AS chef
> > RUN cargo install cargo-chef --locked
> > WORKDIR /app
> >
> > # Planner - create recipe from dependencies only
> > FROM chef AS planner
> > COPY Cargo.toml Cargo.lock ./
> > COPY my-crate/Cargo.toml my-crate/
> > COPY crates crates/
> > RUN cargo chef prepare --recipe-path recipe.json
> >
> > # Builder - build dependencies, then source
> > FROM chef AS builder
> >
> > # Build dependencies (cached if recipe.json unchanged)
> > COPY --from=planner /app/recipe.json recipe.json
> > RUN cargo chef cook --release --recipe-path recipe.json
> >
> > # Build application (only this layer rebuilds on code changes)
> > COPY . .
> > RUN cargo build --release && strip target/release/my-binary
> >
> > # Runtime - minimal image
> > FROM debian:bookworm-slim
> > COPY --from=builder /app/target/release/my-binary /usr/local/bin/
> > CMD ["my-binary"]
> > ```
> >
> > **Key Points:**
> > - Planner stage copies only Cargo.toml files (not source code)
> > - `cargo chef prepare` creates recipe.json from dependencies
> > - `cargo chef cook` compiles dependencies - this layer is cached
> > - Source code is copied after dependencies are built
> > - Only the final `cargo build` recompiles when code changes
> > </details>
> >
> > <details>
> > <summary><strong>GitHub Actions CI Pattern</strong></summary>
> >
> > ```yaml
> > jobs:
> >   build:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >
> >       - uses: docker/setup-buildx-action@v3
> >
> >       - uses: docker/login-action@v3
> >         with:
> >           registry: ${{ env.REGISTRY }}
> >           username: ${{ secrets.REGISTRY_USERNAME }}
> >           password: ${{ secrets.REGISTRY_TOKEN }}
> >
> >       - name: Build image
> >         uses: docker/build-push-action@v6
> >         with:
> >           context: .
> >           file: ./Dockerfile
> >           push: false
> >           load: true
> >           tags: ${{ env.REGISTRY }}/my-image:${{ env.TAG }}
> >           cache-from: |
> >             type=registry,ref=${{ env.REGISTRY }}/my-image:cache
> >           cache-to: |
> >             type=registry,ref=${{ env.REGISTRY }}/my-image:cache,mode=max
> > ```
> >
> > **Key Points:**
> > - `cache-from` pulls cached layers from registry before build
> > - `cache-to` pushes new cache layers after build
> > - `mode=max` caches all intermediate layers (not just final)
> > - Cache tag is separate from image tags (e.g., `:cache`)
> > - Works across different CI runners and branches
> > </details>
> >
> > <details>
> > <summary><strong>What NOT to Do</strong></summary>
> >
> > **Don't use `--mount=type=cache` for CI:**
> > ```dockerfile
> > # BAD - cache doesn't persist between CI runners
> > RUN --mount=type=cache,target=/usr/local/cargo/registry \
> >     cargo build --release
> > ```
> >
> > **Don't copy all source files before dependencies:**
> > ```dockerfile
> > # BAD - any file change invalidates dependency cache
> > COPY . .
> > RUN cargo build --release
> > ```
> >
> > **Don't use GHA cache for large Rust builds:**
> > ```yaml
> > # BAD - GHA cache has 10GB limit, Rust target/ easily exceeds it
> > - uses: actions/cache@v4
> >   with:
> >     path: target/
> >     key: rust-${{ hashFiles('Cargo.lock') }}
> > ```
> > </details>
> >
> > <details>
> > <summary><strong>Performance Impact</strong></summary>
> >
> > | Scenario | Without Caching | With cargo-chef + Registry Cache |
> > |----------|----------------|----------------------------------|
> > | First build | 15-30 min | 15-30 min |
> > | Code change only | 15-30 min | 2-5 min |
> > | Dependency change | 15-30 min | 15-30 min |
> > | No changes | 15-30 min | 30 sec - 1 min |
> >
> > The key insight: most CI runs only change application code, not dependencies.
> > With proper caching, these builds skip 90%+ of compilation time.
> > </details>
>
> </details>

<h2>11. Advanced CI Quality Gates</h2>

> [!TIP]
>
> <p>
>   <strong>Beyond basic tests and lints, professional Rust projects should include license compliance, API stability, MSRV verification, and dependency auditing in CI.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > | Tool | Purpose | When to Use |
> > |------|---------|-------------|
> > | `cargo-deny` | License compliance, duplicate deps, security advisories | Any project with dependencies |
> > | `cargo-semver-checks` | Detect breaking API changes | Libraries published to crates.io |
> > | MSRV check | Verify minimum supported Rust version | Projects with `rust-version` in Cargo.toml |
> > | `cargo-machete` | Find unused dependencies | Reduce bloat, faster builds |
> > | Doctests | Verify documentation examples compile | Projects with `///` doc comments |
> >
> > <details>
> > <summary><strong>cargo-deny: License & Security</strong></summary>
> >
> > **Installation:**
> > ```bash
> > cargo install cargo-deny
> > ```
> >
> > **Configuration (`deny.toml`):**
> > ```toml
> > [advisories]
> > db-path = "~/.cargo/advisory-db"
> > vulnerability = "deny"
> > unmaintained = "warn"
> > yanked = "deny"
> >
> > [licenses]
> > allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "Zlib"]
> > copyleft = "deny"
> > unlicensed = "deny"
> >
> > [bans]
> > multiple-versions = "warn"
> > wildcards = "deny"
> >
> > [sources]
> > unknown-registry = "deny"
> > unknown-git = "deny"
> > ```
> >
> > **CI Integration:**
> > ```yaml
> > - name: Check licenses and advisories
> >   run: cargo deny check
> > ```
> >
> > **Why it matters:**
> > - Prevents accidental GPL/AGPL dependencies in MIT projects
> > - Catches known security vulnerabilities (RustSec)
> > - Warns about duplicate dependency versions (bloat)
> > </details>
> >
> > <details>
> > <summary><strong>cargo-semver-checks: API Stability</strong></summary>
> >
> > **Installation:**
> > ```bash
> > cargo install cargo-semver-checks
> > ```
> >
> > **Usage:**
> > ```bash
> > # Compare against last published version
> > cargo semver-checks check-release
> >
> > # Compare against specific version
> > cargo semver-checks check-release --baseline-version 1.2.0
> > ```
> >
> > **CI Integration:**
> > ```yaml
> > - name: Check semver compliance
> >   if: github.event_name == 'pull_request'
> >   run: |
> >     cargo install cargo-semver-checks
> >     cargo semver-checks check-release
> > ```
> >
> > **What it catches:**
> > - Removing public functions/types (breaking)
> > - Changing function signatures (breaking)
> > - Adding required fields to structs (breaking)
> > - Changing enum variants (breaking)
> >
> > **When to use:** Any library published to crates.io where users depend on your API.
> > </details>
> >
> > <details>
> > <summary><strong>MSRV Check: Minimum Supported Rust Version</strong></summary>
> >
> > **In Cargo.toml:**
> > ```toml
> > [package]
> > rust-version = "1.83"  # MSRV
> > edition = "2024"
> > ```
> >
> > **CI Integration:**
> > ```yaml
> > jobs:
> >   msrv:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - name: Extract MSRV
> >         id: msrv
> >         run: |
> >           MSRV=$(grep '^rust-version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
> >           echo "version=$MSRV" >> $GITHUB_OUTPUT
> >       - uses: dtolnay/rust-toolchain@master
> >         with:
> >           toolchain: ${{ steps.msrv.outputs.version }}
> >       - run: cargo check --all-features
> > ```
> >
> > **Why it matters:**
> > - Edition 2024 requires Rust 1.85+
> > - Users on older Rust versions get clear errors
> > - Prevents accidental use of newer features
> > </details>
> >
> > <details>
> > <summary><strong>cargo-machete: Unused Dependencies</strong></summary>
> >
> > **Installation:**
> > ```bash
> > cargo install cargo-machete
> > ```
> >
> > **Usage:**
> > ```bash
> > cargo machete
> > ```
> >
> > **CI Integration:**
> > ```yaml
> > - name: Check for unused dependencies
> >   run: |
> >     cargo install cargo-machete
> >     cargo machete
> > ```
> >
> > **Benefits:**
> > - Faster compile times
> > - Smaller binary size
> > - Reduced attack surface
> > - Cleaner dependency tree
> > </details>
> >
> > <details>
> > <summary><strong>Doctests: Documentation Examples</strong></summary>
> >
> > **Run doctests explicitly:**
> > ```bash
> > cargo test --doc
> > ```
> >
> > **CI Integration:**
> > ```yaml
> > - name: Run doctests
> >   run: cargo test --doc --all-features
> > ```
> >
> > **Example doctest:**
> > ```rust
> > /// Calculates the sum of two numbers.
> > ///
> > /// # Examples
> > ///
> > /// ```
> > /// use mylib::add;
> > /// assert_eq!(add(2, 3), 5);
> > /// ```
> > pub fn add(a: i32, b: i32) -> i32 {
> >     a + b
> > }
> > ```
> >
> > **Why separate doctests:**
> > - `cargo test` runs unit + integration + doc tests together
> > - Doctests often need different feature flags
> > - Faster feedback when docs change but code doesn't
> > </details>
> >
> > <details>
> > <summary><strong>Complete CI Quality Gate</strong></summary>
> >
> > ```yaml
> > jobs:
> >   quality:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: dtolnay/rust-toolchain@stable
> >         with:
> >           components: clippy, rustfmt
> >
> >       - name: Format check
> >         run: cargo +nightly fmt -- --check
> >
> >       - name: Clippy
> >         run: cargo clippy --all-targets -- -D warnings
> >
> >       - name: Tests
> >         run: cargo test --all-features
> >
> >       - name: Doctests
> >         run: cargo test --doc --all-features
> >
> >       - name: Unused dependencies
> >         run: |
> >           cargo install cargo-machete
> >           cargo machete
> >
> >       - name: License & security
> >         run: |
> >           cargo install cargo-deny
> >           cargo deny check
> >
> >   msrv:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: dtolnay/rust-toolchain@1.83.0
> >       - run: cargo check --all-features
> >
> >   semver:
> >     if: github.event_name == 'pull_request'
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: dtolnay/rust-toolchain@stable
> >       - run: |
> >           cargo install cargo-semver-checks
> >           cargo semver-checks check-release
> > ```
> > </details>
>
> </details>

<h2>12. CI/CD Architecture: Single Workflow, Multiple Jobs</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Use a single CI workflow file with multiple jobs instead of multiple separate workflow files. This provides better control, visibility, and resource management.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > **The Problem with Multiple Workflows:**
> > - No way to synchronize jobs between different workflows
> > - Cannot define dependencies (Job C runs after Job A and B)
> > - Harder to manage concurrency and cancellation
> > - Duplicated trigger configuration across files
> > - Scattered CI logic makes debugging difficult
> > - Multiple workflow runs for same commit consume more resources
> >
> > **The Solution: Single Workflow with Multiple Jobs**
> >
> > 1. **One workflow file** contains all CI/CD logic
> > 2. **Jobs** handle different tasks (test, build, deploy)
> > 3. **`needs`** keyword defines job dependencies
> > 4. **Reusable workflows** (`_*.yml`) extract common patterns
> > 5. **Concurrency groups** prevent duplicate runs
> >
> > <details>
> > <summary><strong>Architecture Pattern</strong></summary>
> >
> > ```
> > .github/workflows/
> > ├── ci.yml                    # Main CI workflow (triggers on push/PR)
> > ├── _build-service.yml        # Reusable: build Docker image
> > ├── _deploy-service.yml       # Reusable: deploy to k8s
> > └── _quality-check.yml        # Reusable: run tests/lints
> > ```
> >
> > **Key principle:** Files starting with `_` are reusable workflows called via `uses:`. Only `ci.yml` defines triggers.
> > </details>
> >
> > <details>
> > <summary><strong>Job Dependencies with `needs`</strong></summary>
> >
> > ```yaml
> > jobs:
> >   detect-changes:
> >     runs-on: ubuntu-latest
> >     outputs:
> >       api: ${{ steps.filter.outputs.api }}
> >       client: ${{ steps.filter.outputs.client }}
> >
> >   quality-check:
> >     needs: [detect-changes]
> >     if: needs.detect-changes.outputs.api == 'true'
> >
> >   build-api:
> >     needs: [detect-changes, quality-check]
> >     if: |
> >       always() &&
> >       needs.detect-changes.outputs.api == 'true' &&
> >       needs.quality-check.result == 'success'
> >
> >   deploy-api:
> >     needs: [build-api]
> >     if: needs.build-api.result == 'success'
> > ```
> >
> > **Key Points:**
> > - `needs` creates dependency chain
> > - Jobs run in parallel unless `needs` enforces order
> > - Use `if: always()` to run even if dependencies were skipped
> > - Check `needs.<job>.result` for conditional execution
> > </details>
> >
> > <details>
> > <summary><strong>Concurrency Control</strong></summary>
> >
> > ```yaml
> > name: CI/CD Pipeline
> >
> > on:
> >   push:
> >     branches: [main]
> >   pull_request:
> >
> > concurrency:
> >   group: ci-${{ github.ref }}
> >   cancel-in-progress: true
> > ```
> >
> > **What this does:**
> > - Groups runs by branch/PR (`github.ref`)
> > - New push cancels previous running workflow
> > - Prevents wasted resources on outdated commits
> > - Only one active run per branch at a time
> > </details>
> >
> > <details>
> > <summary><strong>Reusable Workflows</strong></summary>
> >
> > **Main workflow calls reusable:**
> > ```yaml
> > # ci.yml
> > jobs:
> >   build-api:
> >     uses: ./.github/workflows/_build-service.yml
> >     with:
> >       service_name: api-server
> >       dockerfile: ./api-server/Dockerfile
> >     secrets:
> >       registry_token: ${{ secrets.REGISTRY_TOKEN }}
> > ```
> >
> > **Reusable workflow definition:**
> > ```yaml
> > # _build-service.yml
> > name: Build Service
> >
> > on:
> >   workflow_call:
> >     inputs:
> >       service_name:
> >         required: true
> >         type: string
> >       dockerfile:
> >         required: true
> >         type: string
> >     secrets:
> >       registry_token:
> >         required: true
> >     outputs:
> >       image_tag:
> >         value: ${{ jobs.build.outputs.tag }}
> >
> > jobs:
> >   build:
> >     runs-on: ubuntu-latest
> >     outputs:
> >       tag: ${{ steps.meta.outputs.tag }}
> >     steps:
> >       # ... build logic
> > ```
> >
> > **Benefits:**
> > - DRY: same build logic for all services
> > - Inputs/outputs for configuration
> > - Secrets passed explicitly (security)
> > - Easy to update in one place
> > </details>
> >
> > <details>
> > <summary><strong>Independent Service Builds</strong></summary>
> >
> > ```yaml
> > jobs:
> >   build-api:
> >     needs: [detect-changes, quality-api]
> >     if: needs.detect-changes.outputs.api == 'true'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   build-client:
> >     needs: [detect-changes, quality-client]
> >     if: needs.detect-changes.outputs.client == 'true'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   deploy-api:
> >     needs: [build-api]  # Only depends on its own build
> >     if: needs.build-api.result == 'success'
> >
> >   deploy-client:
> >     needs: [build-client]  # Independent from api
> >     if: needs.build-client.result == 'success'
> > ```
> >
> > **Key principle:** Each service's deploy depends only on its own build, not on other services. If api-server build fails, client can still deploy.
> > </details>
> >
> > <details>
> > <summary><strong>What NOT to Do</strong></summary>
> >
> > **Don't create separate workflow files for each task:**
> > ```
> > # BAD - no synchronization possible
> > .github/workflows/
> > ├── test.yml
> > ├── build-api.yml
> > ├── build-client.yml
> > ├── deploy-api.yml
> > ├── deploy-client.yml
> > └── cleanup.yml
> > ```
> >
> > **Don't make all deploys depend on all builds:**
> > ```yaml
> > # BAD - client waits for api even if unrelated
> > deploy-client:
> >   needs: [build-api, build-client, build-worker]
> > ```
> >
> > **Don't skip concurrency control:**
> > ```yaml
> > # BAD - multiple runs waste resources
> > on:
> >   push:
> >     branches: [main]
> > # Missing: concurrency group
> > ```
> > </details>
> >
> > <details>
> > <summary><strong>Complete Example Structure</strong></summary>
> >
> > ```yaml
> > name: CI/CD Pipeline
> >
> > on:
> >   push:
> >     branches: [main]
> >   workflow_dispatch:
> >     inputs:
> >       deploy_all:
> >         type: boolean
> >         default: false
> >
> > concurrency:
> >   group: ci-${{ github.ref }}
> >   cancel-in-progress: true
> >
> > jobs:
> >   # 1. Detect what changed
> >   detect-changes:
> >     runs-on: ubuntu-latest
> >     outputs:
> >       api: ${{ steps.filter.outputs.api }}
> >       client: ${{ steps.filter.outputs.client }}
> >     steps:
> >       - uses: dorny/paths-filter@v3
> >         id: filter
> >         with:
> >           filters: |
> >             api:
> >               - 'api-server/**'
> >             client:
> >               - 'client/**'
> >
> >   # 2. Quality gates (parallel)
> >   quality-api:
> >     needs: [detect-changes]
> >     if: needs.detect-changes.outputs.api == 'true'
> >     uses: ./.github/workflows/_quality-check.yml
> >
> >   quality-client:
> >     needs: [detect-changes]
> >     if: needs.detect-changes.outputs.client == 'true'
> >     uses: ./.github/workflows/_quality-check.yml
> >
> >   # 3. Build (after quality)
> >   build-api:
> >     needs: [detect-changes, quality-api]
> >     if: needs.quality-api.result == 'success'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   build-client:
> >     needs: [detect-changes, quality-client]
> >     if: needs.quality-client.result == 'success'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   # 4. Deploy (independent per service)
> >   deploy-api:
> >     needs: [build-api]
> >     if: needs.build-api.result == 'success'
> >     uses: ./.github/workflows/_deploy-service.yml
> >
> >   deploy-client:
> >     needs: [build-client]
> >     if: needs.build-client.result == 'success'
> >     uses: ./.github/workflows/_deploy-service.yml
> > ```
> > </details>
>
> </details>

<p align=center>
  <em>Following these guidelines ensures that our Rust code is high-quality, maintainable, and scalable.</em>
</p>
