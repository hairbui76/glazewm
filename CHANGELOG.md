# Changelog

## [3.11.0](https://github.com/hairbui76/glazewm/compare/v3.10.1...v3.11.0) (2026-09-16)


### Features

* add "Check for updates" to the system tray ([51f22d9](https://github.com/hairbui76/glazewm/commit/51f22d970b843e8704c064a64270619b3f8c21dd))
* add ignore rule for CleanShot X to sample config ([#1343](https://github.com/hairbui76/glazewm/issues/1343)) ([9d94e12](https://github.com/hairbui76/glazewm/commit/9d94e125fb7b520559227bf1f4ffa09802d520d0))
* add one-line install script for Windows ([befbe98](https://github.com/hairbui76/glazewm/commit/befbe9820d5fe4e136f617ad971dac27eb6d3de1))
* add reciprocal keybind to sample config for exiting resize mode ([#1344](https://github.com/hairbui76/glazewm/issues/1344)) ([efd0efa](https://github.com/hairbui76/glazewm/commit/efd0efaa8d58d832ac9a7a6ba63968e91d2f9702))
* Minimized windows to workspaces when startup ([aae47df](https://github.com/hairbui76/glazewm/commit/aae47dfaebe2189a91b607022c4fe2e51cd4df85))
* Support toggle for workspaces in single monitor only ([801f698](https://github.com/hairbui76/glazewm/commit/801f6984f0ca131c278719b80c72b2e4f398520c))


### Bug fixes

* capture hide events outside of the visible workspaces on macOS ([#1359](https://github.com/hairbui76/glazewm/issues/1359)) ([dce8d14](https://github.com/hairbui76/glazewm/commit/dce8d143143c4a9268347016a1fdf03552c99b2f))
* Change primary display name to hardwareId filter ([2c32b0c](https://github.com/hairbui76/glazewm/commit/2c32b0cbf307edc55ffadcf98d60060be34b4f55))
* **ci:** publish releases directly and skip signing without secrets ([6263fd1](https://github.com/hairbui76/glazewm/commit/6263fd1bef6ed77c69aa61bcdc6decb165de46ec))
* clean up stale entries from ignored windows list ([#1312](https://github.com/hairbui76/glazewm/issues/1312)) ([ddf7bc1](https://github.com/hairbui76/glazewm/commit/ddf7bc19b6c7c692ff388ea2bdff3526efd22a7e))
* enforce single-monitor workspace placement across all entry points ([1b68c27](https://github.com/hairbui76/glazewm/commit/1b68c2793a1baa73fab70f004e5d80b2a2392373))
* Prevent monitor-to-monitor window flicker on display change ([54654a3](https://github.com/hairbui76/glazewm/commit/54654a3ffb34da713868182e7298e4ec57b59de4))
* Refine window placement on unmanaged monitors ([41e466e](https://github.com/hairbui76/glazewm/commit/41e466ed8c02273d5d7b7f8ff1179604bdfd145b))
* segfault on focus with macOS sonoma 14.5 ([#1340](https://github.com/hairbui76/glazewm/issues/1340)) ([919f4c9](https://github.com/hairbui76/glazewm/commit/919f4c93e32f7c93e66a745f9ab8e434dac968a0))


### Refactors

* remove macOS support ([6ed9dd7](https://github.com/hairbui76/glazewm/commit/6ed9dd7ade09caa63e6c86a74dc470dee84b5e6f))
