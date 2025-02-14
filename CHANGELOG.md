# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org).

<!--
Note: In this file, do not use the hard wrap in the middle of a sentence for compatibility with GitHub comment style markdown rendering.
-->

## [Unreleased]

## [0.2.0] - 2021-04-17

- [Support Dependabot v1 configuration file.](https://github.com/taiki-e/dependabot-config/pull/3)

- [Add `v2::CommitMessageInclude` and `v2::InsecureExternalCodeExecution`.](https://github.com/taiki-e/dependabot-config/pull/3)

- [Change `v2::CommitMessage::include` field from `Option<String>` to `Option<v2::CommitMessageInclude>`.](https://github.com/taiki-e/dependabot-config/pull/3)

- [Change `v2::Update::insecure_external_code_execution` field from `Option<String>` to `Option<v2::InsecureExternalCodeExecution>`.](https://github.com/taiki-e/dependabot-config/pull/3)

- [Implement `Display` for `v2::{PackageEcosystem, Interval, Day, DependencyType, RebaseStrategy, VersioningStrategy, RegistryType, Separator}`.](https://github.com/taiki-e/dependabot-config/pull/3)

## [0.1.0] - 2021-04-09

Initial release

[Unreleased]: https://github.com/taiki-e/dependabot-config/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/taiki-e/dependabot-config/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/taiki-e/dependabot-config/releases/tag/v0.1.0
