<h1>Rust Code Style Guide</h1>

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
> > <pre><code>cargo +nightly fmt</code></pre>
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
>   <strong>Use a professional branching strategy to ensure efficient and conflict-free collaboration.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
> 
> > - **Check Existing Branches:** Review current active branches to work on the latest relevant code.
> > - **Clarify When in Doubt:** If unsure, confirm the correct branch via team chat or project guidelines to avoid conflicts.
> > - **Create a Descriptive Branch:** Name branches clearly, e.g., <code>feature/123-add-user-authentication</code> or <code>bugfix/456-fix-crash-on-login</code>.
> > - **Commit and Push Regularly:** Commit with clear messages and push frequently to back up work and keep the team informed.
> > - **Create a Pull Request (PR):** Open a PR from your branch with clear issue references to provide context for reviewers.
> > - **Clean Up After Merge:** Delete merged branches to maintain a tidy repository.
> > - **Minimize Excessive Communication:** Use clear branch names, detailed commits, and well-documented PRs to reduce the need for frequent ad hoc discussions.
> >
> >
> > <details>
> > <summary><strong>Real-World Example &amp; Further Explanation</strong></summary>
> > <p>
> >   For example, if you start work on a new feature, check existing branches to avoid duplicating work. If uncertain, ask in the team channel or check the project topic. Once confirmed, create a branch like <code>feature/123-add-user-authentication</code>, commit changes with detailed messages, and open a PR referencing the issue number. This method reduces unnecessary phone calls and streamlines collaboration.
> > </p>
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


<h2>6. GitHub Projects and Weekly Reports</h2>

> [!NOTE]
>
> <p>
>   <strong>Use GitHub Projects for task tracking and submit weekly reports.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
> 
> > - Use GitHub Projects to track tasks and milestones.
> > - Each task must have a clear description, priority, and assignee.
> > - Weekly reports must be submitted between Friday and Sunday.<br>
> >   <a href="https://docs.google.com/forms/d/e/1FAIpQLScZOcUixej1cyRtmf1CC264Ylfk7tRHIGu-7Xq-9-X9TkfkxQ/viewform">Send weekly report</a>
> > - The report should include completed tasks, ongoing work, and blockers.
> >
> >
> > <details>
> > <summary><strong>Why This Matters</strong></summary>
> > <p>
> >   Structured task tracking via GitHub Projects provides clarity and accountability. Regular weekly reports help identify progress and blockers early, facilitating better planning and resource allocation.
> > </p>
> > </details>
> 
> </details>


<h2>7. Panics in Rust</h2>

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
> 
> </details>

<p>
  Following these guidelines ensures that our Rust code is high-quality, maintainable, and scalable.
</p>
