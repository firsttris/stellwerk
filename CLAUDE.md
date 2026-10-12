# Notes for Claude

## Documentation

The documentation is in `docs/` (<https://firsttris.github.io/stellwerk/>). Read the pages that
touch the change before starting: `architecture.md` for how Zentrale, hub, agent and engine fit
together and for the roadmap. Links in the README and the docs point to the GitHub Pages site, not
to `docs/*.md`.

## Reference implementations

Base details on the sources, not on memory: the
[Syncthing REST API](https://docs.syncthing.net/dev/rest.html) and Syncthing's source, above all
`lib/config` (configuration types, defaults, `prepare`) and `lib/api` (`confighandler.go` for
what `PUT`/`PATCH`/`DELETE` on `/rest/config/...` do). Clone them shallowly
(`git clone --depth 1 https://github.com/syncthing/syncthing`) into the scratchpad, check out the
tag pinned in `crates/engine/src/pin.rs`, and name the source file in comments and commit messages
where it settles a detail.

## With every change

Update what the change affects, in the same pull request, and only that:

- the tests for what changed (unit tests for pure functions such as the reconciler, integration
  tests in `crates/engine/tests` against the pinned Syncthing; later Vitest and Playwright for
  `web/`);
- the docs in `docs/` that describe it;
- the README when a feature a user sees is added, changed or removed;
- the screenshots after a change to the look: **Actions → Update screenshots → Run workflow**, then
  look at the new pictures.

## Checks

Green before every push, then check CI:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test                # downloads the pinned Syncthing once into .syncthing-cache/
mkdocs build --strict     # when docs/ or mkdocs.yml changed
```

## Pull requests

- One feature per pull request; it carries only its own commits.
- Before every push check whether the branch's pull request is still open. If it is merged, start
  again from the current `origin/main` and open a **new** pull request.
- Merge method: squash. Force-push: allowed with `--force-with-lease` on the own branch after a
  merge.

## Working rules

- Commands as root (`sudo`, `pkexec`) are run by the user; don't work around that.
- The user's running processes are not stopped; use other ports for tests.
- Language: code, comments, docs, commits and pull requests in English; the app's interface in
  German and English.

## Code style

Technology choices follow `tech-stack.md` in firsttris/workflows, with one deviation: the
Zentrale is written in **Rust (axum)**, not TanStack Start, because agent and Zentrale share the
crates `protocol` (messages, desired state) and `engine` (Syncthing control; the Zentrale steers
its hub Syncthing with the same code as the agents). One crate for both means less code and no
type drift. The UI in `web/` follows the tech stack (React, TanStack, Tailwind, shadcn style).

Clean Code, written functionally: clear names, small functions that do one thing, dependencies
passed in, no duplication or dead code, comments that say why; the ideas behind SOLID and the
design patterns are welcome as functions, data and closures, but not as classes, inheritance or
class hierarchies.

Functional, not object-oriented: pure functions that take data and return data, side effects at
the edges; function components and hooks, no classes; errors as values. A file does one thing:
split it past ~400 lines, a function past ~80. A refactor changes no behaviour.

## Stellwerk specifics

- **Never start a test Syncthing on the standard ports 8384 (GUI) or 22000 (sync).** Use
  `engine::process::SpawnOptions::isolated`, which takes random ports on 127.0.0.1 from the OS
  and turns discovery, relays and NAT off.
- **Never touch a Syncthing that is already running** (the user's own, or one from another
  test): no `pkill syncthing`, no access to its `--home` or API. Stop only the instances you
  started, through `Instance::stop`.
- `engine` is the only crate that knows Syncthing; `protocol` describes devices, folders, modes
  and paths only, so a different engine can replace `engine` later.
- The reconciler (`engine::reconcile::plan`) stays a pure function: actual config + desired
  state → list of API calls. Side effects only in `engine::apply`. It changes configuration,
  never files.
- The Syncthing version is pinned in `crates/engine/src/pin.rs` with SHA-256 per platform;
  raising it means new checksums and a green integration test.
