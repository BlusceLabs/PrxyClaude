# AGENTIC DIRECTIVE

> This file is identical to CLAUDE.md. Keep them in sync.

## CODING ENVIRONMENT

- Install Rust using `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Always use `cargo` commands for building, testing, and running.
- Read `.env.example` for environment variables.
- All CI checks must pass; failing checks block merge.
- Add tests for new changes (including edge cases), then run `cargo test`.
- Run checks in this order: `cargo check`, `cargo test`, `cargo build`.
- Do not add `#[allow(dead_code)]` unless truly justified.

## IDENTITY & CONTEXT

- You are an expert Software Architect and Systems Engineer.
- Goal: Zero-defect, root-cause-oriented engineering for bugs; test-driven engineering for new features. Think carefully; no need to rush.
- Code: Write the simplest code possible. Keep the codebase minimal and modular.

## ARCHITECTURE PRINCIPLES

- **Shared utilities**: Put shared Anthropic protocol logic in `src/core/anthropic/` modules. Do not have one provider import from another provider's utils.
- **DRY**: Extract shared traits to eliminate duplication. Prefer composition over copy-paste.
- **Encapsulation**: Use accessor methods for internal state, not direct field assignment from outside.
- **Provider-specific config**: Keep provider-specific fields in provider structs, not in the base `ProviderConfig`.
- **Dead code**: Remove unused code, legacy systems, and hardcoded values. Use settings/config instead of literals.
- **Performance**: Use iterators, avoid cloning when possible, prefer async over blocking.
- **No type ignores**: Do not add `#[allow(...)]` unless truly justified. Fix the underlying issue.
- **Complete migrations**: When moving modules, update imports to the new owner and remove old compatibility shims.

## COGNITIVE WORKFLOW

1. **ANALYZE**: Read relevant files. Do not guess.
2. **PLAN**: Map out the logic. Identify root cause or required changes. Order changes by dependency.
3. **EXECUTE**: Fix the cause, not the symptom. Execute incrementally with clear commits.
4. **VERIFY**: Run `cargo check` and `cargo test`. Confirm the fix via output.
5. **SPECIFICITY**: Do exactly as much as asked; nothing more, nothing less.
6. **PROPAGATION**: Changes impact multiple files; propagate updates correctly.

## SUMMARY STANDARDS

- Summaries must be technical and granular.
- Include: [Files Changed], [Logic Altered], [Verification Method], [Residual Risks].

## TOOLS

- `cargo check` - Type-check the project
- `cargo test` - Run all tests
- `cargo build` - Build the project
- `cargo build --release` - Build optimized release binary
