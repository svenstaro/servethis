# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->

## [Unreleased] - ReleaseDate
- Add hardened systemd template unit file to `packaging/miniserve@.service`

## [0.14.0] - 2021-04-18
- Fix breadcrumbs for right-to-left languages [#489](https://github.com/svenstaro/miniserve/pull/489) (thanks @aliemjay)
- Fix URL percent encoding for special characters [#485](https://github.com/svenstaro/miniserve/pull/485) (thanks @aliemjay)
- Wrap breadcrumbs at any char [#496](https://github.com/svenstaro/miniserve/pull/496) (thanks @aliemjay)
- Add separate flags for compressed and uncompressed tar archives [#492](https://github.com/svenstaro/miniserve/pull/492) (thanks @deantvv)
- Bump deps
- Fix Firefox becoming confused when opening a `.gz` file directly [#160](https://github.com/svenstaro/miniserve/issues/160)
- Prefer UTF8 for text responses [#263](https://github.com/svenstaro/miniserve/issues/263)
- Resolve symlinks on directory listing [#479](https://github.com/svenstaro/miniserve/pull/479) (thanks @aliemjay)

## [0.13.0] - 2021-03-28
- Change default log level to `Warn`
- Change some messages a bit to be more clear
- Add `--print-completions` to print shell completions for various supported shells [#482](https://github.com/svenstaro/miniserve/pull/482) (thanks @rouge8)
- Don't print some messages if not attached to an interactive terminal
- Refuse to start if not attached to interactive terminal and no explicit path is provided

  This is a security consideration as you wouldn't want to run miniserve without an explicit path
  as a service. You could end up serving `/` or `/root` in case those working directories are set.

## [0.12.1] - 2021-03-27
- Fix QR code not showing when using both `--random-route` and `--qrcode` [#480](https://github.com/svenstaro/miniserve/pull/480) (thanks @rouge8)
- Add FreeBSD binaries

## [0.12.0] - 2021-03-20
- Add option `-H`/`--hidden` to show hidden files
- Start instantly in case an explicit index is chosen
- Fix DoS issue when deliberately sending unconforming URL paths
- Add footer [#456](https://github.com/svenstaro/miniserve/pull/456) (thanks @levaitamas)
- Switched from failure to thiserror for error handling

## [0.11.0] - 2021-02-28
- Add binaries for more architectures
- Upgrade lockfile which fixes some security issues
- Allow multiple file upload [#434](https://github.com/svenstaro/miniserve/pull/434) (thanks @mhuesch)
- Allow for setting custom headers via `--header` [#452](https://github.com/svenstaro/miniserve/pull/452) (thanks @deantvv)

## [0.10.4] - 2021-01-05
- Add `--dirs-first`/`-D` option to list directories first [#423](https://github.com/svenstaro/miniserve/pull/423) (thanks @levaitamas)

## [0.10.3] - 2020-11-09
- Actually fix publish workflow

## [0.10.2] - 2020-11-09
- Fix publish workflow

## [0.10.1] - 2020-11-09
- Now compiles on stable! :D

## [0.10.0] - 2020-10-02
- Add embedded favicon [#364](https://github.com/svenstaro/miniserve/issues/364)
- Add `--title` option which can be used to set the page title [#378](https://github.com/svenstaro/miniserve/pull/378) (thanks @ahti)
- Default title is now the same host received in the request [#378](https://github.com/svenstaro/miniserve/pull/378) (thanks @ahti)
- Client-side color-scheme handling [#380](https://github.com/svenstaro/miniserve/pull/380) (thanks @ahti)

## [0.9.0] - 2020-09-16
- Added prebuilt binaries for AARCH64, ARMv7, and ARM [#350](https://github.com/svenstaro/miniserve/pull/350)
- Remove percent-encoding in heading and title [#362](https://github.com/svenstaro/miniserve/pull/362) (thanks @ahti)
- Make name ordering case-insensitive [#362](https://github.com/svenstaro/miniserve/pull/362) (thanks @ahti)
- Give name column more space [#362](https://github.com/svenstaro/miniserve/pull/362) (thanks @ahti)
- Fix double-escaping [#354](https://github.com/svenstaro/miniserve/issues/354)
- Upgrade to actix-web 3.0
- Fix time display for files created "now" [#373](https://github.com/svenstaro/miniserve/pull/373) (thanks @imp and @KevCui)

## [0.8.0] - 2020-07-22
- Accept port 0 to find a random free port and use that [#327](https://github.com/svenstaro/miniserve/pull/327) (thanks @parrotmac)
- Show QR code in interface [#330](https://github.com/svenstaro/miniserve/pull/330) (thanks @wyhaya)
- Ported to actix-web 2 and futures 0.3 [#343](https://github.com/svenstaro/miniserve/pull/343) (thanks @equal-l2)

## [0.7.0] - 2020-05-14
- Add zip archiving [#297](https://github.com/svenstaro/miniserve/pull/297) (thanks @marawan31)

## [0.6.0] - 2020-03-14
- Add option to disable archives [#235](https://github.com/svenstaro/miniserve/pull/235) (thanks @DamianX)
- Fix minor bug when using `--random-route` [#219](https://github.com/svenstaro/miniserve/pull/219)
- Add a default index serving option [#189](https://github.com/svenstaro/miniserve/pull/189)

## [0.5.0] - 2019-06-24
- Add streaming download of tar archives (thanks @gyscos)
- Add support for hashed passwords (thanks @KSXGitHub)
- Add support for multiple auth flags (thanks @KSXGitHub)
- Some theme related bug fixes (thanks @boastful-squirrel)

<!-- next-url -->
[Unreleased]: https://github.com/svenstaro/miniserve/compare/v0.14.0...HEAD
[0.14.0]: https://github.com/svenstaro/miniserve/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/svenstaro/miniserve/compare/v0.12.1...v0.13.0
[0.12.1]: https://github.com/svenstaro/miniserve/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/svenstaro/miniserve/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/svenstaro/miniserve/compare/v0.10.4...v0.11.0
[0.10.4]: https://github.com/svenstaro/miniserve/compare/v0.10.3...v0.10.4
[0.10.3]: https://github.com/svenstaro/miniserve/compare/v0.10.2...v0.10.3
[0.10.2]: https://github.com/svenstaro/miniserve/compare/v0.10.1...v0.10.2
[0.10.1]: https://github.com/svenstaro/miniserve/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/svenstaro/miniserve/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/svenstaro/miniserve/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/svenstaro/miniserve/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/svenstaro/miniserve/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/svenstaro/miniserve/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/svenstaro/miniserve/compare/v0.4.0...v0.5.0
