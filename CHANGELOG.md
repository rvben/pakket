# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).


## [0.1.1](https://github.com/rvben/pakket/compare/v0.1.0...v0.1.1) - 2026-06-20

### Added

- **schema**: adopt clispec v0.2 contract ([8d9cae9](https://github.com/rvben/pakket/commit/8d9cae975c615fcc5837f68f66fcd0f8119d4769))

## [0.1.0] - 2026-06-20

### Added

- **pakket**: fall back to direct carrier when 17track returns Pending ([4202e67](https://github.com/rvben/pakket/commit/4202e6757794e67af9631a2300bad22cf86bf018))
- **pakket**: improve config init with guided setup ([6b2a1a7](https://github.com/rvben/pakket/commit/6b2a1a71271050adc83b43c02dfc0d402693fc8f))
- **pakket**: multi-backend carrier routing and config ([ed87077](https://github.com/rvben/pakket/commit/ed8707743f56a393caf59acb78846ce6085f785d))
- **pakket**: add PostNL and DHL carrier backends ([814b664](https://github.com/rvben/pakket/commit/814b66479805ab49e230fea9ec1c1db6587f6df8))
- **pakket**: add track, add, list, and remove commands ([7b264e1](https://github.com/rvben/pakket/commit/7b264e13e2dcf6a6969ca610cf93ac7ac890d4c9))
- **pakket**: add config, output, shipments, and schema modules ([f7edc2a](https://github.com/rvben/pakket/commit/f7edc2a9e8cb63456ac92738d089b68a96b29848))
- **pakket**: add carrier trait and 17track API client ([6cbba77](https://github.com/rvben/pakket/commit/6cbba77c2bc44212b7c629fd97e553dacbc85dc2))
- **pakket**: add error types with exit codes ([6ff8a4d](https://github.com/rvben/pakket/commit/6ff8a4d83d2c6a0c20ff8c45dbd7ec09953febcb))
- **pakket**: scaffold project with CLI structure ([d070f43](https://github.com/rvben/pakket/commit/d070f4316d44012c2f9644622d5ac608b547beef))

### Fixed

- **pakket**: improve table formatting with proper column spacing ([bd5c5fd](https://github.com/rvben/pakket/commit/bd5c5fdc6d007a44d93b7154d9510cc85fa1f436))
- **pakket**: rewrite 17track parser for actual v2.2 response format ([62b1616](https://github.com/rvben/pakket/commit/62b16160be2f5d6e6960146d86889fbe70d10b87))
- **pakket**: align PostNL parser with actual API response ([31bfdd9](https://github.com/rvben/pakket/commit/31bfdd97adc527176db7f660cbb8dd26b64b3797))
