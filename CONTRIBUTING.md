# Contributing

## Ground rules

- **Write it yourself.** Don't copy or port code, text or assets from other projects,
  especially other mod managers or XXMI. Implement from the specs in `docs/`. If you need a new
  fact about a game or XXMI, observe the behaviour and add it to `docs/03-game-adapters.md`
  first. See `docs/06-licensing-and-provenance.md`.
- **Security rules are not optional.** Read `docs/04-security.md`. PRs that add a network host,
  a filesystem root, a process launch, a Cargo feature, or a dependency with a build script need
  the `security-review` label.
- **No telemetry.** Don't add analytics, crash uploaders or "phone home" features.

## Sign-off

Every commit must be signed off (`git commit -s`), certifying the
[Developer Certificate of Origin](https://developercertificate.org/).

## License of contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed under MIT OR
Apache-2.0, without any additional terms or conditions.
