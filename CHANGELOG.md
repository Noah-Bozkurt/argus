# Changelog

All notable operator-visible changes to Argus are documented here.

The project is under active development and does not yet promise semantic-version compatibility. Entries follow the structure of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) where practical.

## [Unreleased]

### Added

- Repository governance, contribution, security, design, agent guidance, and GitHub issue/PR templates.
- Actionable Docker image-pull diagnostics for normal and verbose transactional updates.

### Changed
- Relicensed Argus from its former proprietary license to the GNU Affero General Public License v3.0 only.


- Installer, repair, branch update, and transactional update flows now use the public GitHub repository and public GHCR packages without stored GitHub package credentials.
- Repository documentation and installer guidance now describe anonymous public package access.

### Removed

- GitHub/GHCR package credential prompts, `registry-login` commands, and the legacy registry-login compatibility script.
- Runtime use of `/etc/argus/registry.env`; existing files are removed during lifecycle operations without being read or logged.

## Historical changes

Changes before this changelog was introduced are preserved in the Git history and merged pull requests. Versioned release sections will be added when Argus begins publishing named releases.
