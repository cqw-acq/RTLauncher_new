# Dual-Source Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Check Lighting-Team and GitHub update sources, select the highest stable semantic version, and limit non-forced checks to once every 60 seconds.

**Architecture:** Parse the single-release Lighting-Team response and the release-array GitHub response into one internal release model. Query both sources, keep successful results, select the highest stable semantic version above the installed Tauri package version, and use the existing platform selection, download, and install path.

**Tech Stack:** Rust, Serde, Reqwest, Semver, Tauri 2

## Global Constraints

- Use `http://update-service.lighting-team.com/api/v1/versions` as the primary metadata service.
- Keep `https://api.github.com/repos/cqw-acq/RTLauncher_new/releases` as an independent fallback source.
- Select the highest stable semantic version. Never offer a version equal to or lower than the installed version.
- Limit non-forced checks to once every 60 seconds. Forced startup checks stay unchanged.
- Accept downloads only from GitHub or `tcb.qcloud.la` hosts.
- Keep SHA-256 verification and existing platform installation behavior.
- Do not create a Git commit.

---

### Task 1: Source configuration and cooldown

**Files:**
- Modify: `src-tauri/src/updater/config.rs`

**Interfaces:**
- Produces: `get_update_endpoints() -> [&'static str; 2]`
- Produces: `has_check_interval_elapsed(last_check: i64, now: i64) -> bool`

- [ ] Write tests that reject a second check at 59 seconds, accept it at 60 seconds, accept GitHub and Tencent Cloud download URLs, and reject GitCode URLs.
- [ ] Run `cargo test updater::config::tests --lib` from `src-tauri` and confirm that the new assertions fail.
- [ ] Set the cooldown to 60 seconds, expose both metadata endpoints, and update the trusted download hosts.
- [ ] Run `cargo test updater::config::tests --lib` and confirm that it passes.

### Task 2: Normalize both response formats

**Files:**
- Modify: `src-tauri/src/updater/fetcher.rs`

**Interfaces:**
- Consumes: Lighting-Team single-release JSON and GitHub release-array JSON.
- Produces: internal normalized releases with text notes, checksum maps, optional platform values, assets, and parsed semantic versions.

- [ ] Write tests with complete fixtures from both APIs. Verify structured Lighting-Team notes and hashes, GitHub text bodies, and real attachment file names.
- [ ] Run `cargo test updater::fetcher::tests --lib` and confirm that parsing tests fail.
- [ ] Add Serde wire types and pure normalization functions.
- [ ] Run the fetcher tests and confirm that parsing tests pass.

### Task 3: Select the newest safe update

**Files:**
- Modify: `src-tauri/src/updater/fetcher.rs`
- Modify: `src-tauri/src/updater/handler.rs`

**Interfaces:**
- Produces: selection of the highest stable version that is strictly greater than the installed Tauri package version.
- Consumes: `AppHandle::package_info().version` as the installed version.

- [ ] Write tests that select `1.2.0` over `1.1.9`, ignore prereleases, and return no update when the remote version is equal to or lower than the installed version.
- [ ] Run the tests and confirm that selection tests fail.
- [ ] Query both endpoints, retain results from either successful source, normalize all releases, and select the highest eligible version.
- [ ] Pass the installed Tauri package version from the command handler to the fetcher.
- [ ] Run the tests and confirm that selection tests pass.

### Task 4: Remove the fixed target release

**Files:**
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/updater/config.rs`
- Modify: `src-tauri/src/updater/handler.rs`
- Modify: `src-tauri/src/lib.rs`
- Delete: `src-tauri/update_target.json`

**Interfaces:**
- Removes: `UPDATE_TARGET_RELEASE_NAME`, `get_target_release_name()`, and the unused `get_target_version` Tauri command.

- [ ] Remove fixed-target reads and command registration.
- [ ] Keep `tauri_build::build()` as the build script entry point.
- [ ] Search the workspace and confirm that no production code reads `UPDATE_TARGET_RELEASE_NAME` or `target_release_name`.

### Task 5: Verification

**Files:**
- Verify all modified Rust files.

- [ ] Run `cargo fmt --all -- --check` from `src-tauri`.
- [ ] Run `cargo test updater:: --lib` from `src-tauri`.
- [ ] Run `cargo check --lib` from `src-tauri`.
- [ ] Run `git diff --check` from the repository root.
- [ ] Inspect `git diff` and confirm that no Git commit was created.
