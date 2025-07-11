# Journey to Poplar v0.1
I've been thinking it'd be nice to polish the various bits we have mostly-working now and call it a first version
of Poplar. Having a distinct first version that has baseline functionality will be helpful, I think, in deciding
next steps for progression and stop the somewhat-aimless development we've had at times over the last 8(!!) years.

## Seed
- [ ] Migrate UEFI loader to new version of `uefi` crate
- [ ] Stop using custom memory types in UEFI loader (some firmwares don't like it)
- [ ] Sort out dynamic mapping of physical memory map with high addresses
- [ ] Split out Seed interface into separate crate (this loosens dependencies between Seed and kernel)
- [ ] Move config parsing, image loading functionality, etc. to main `seed` crate
- [ ] Move to giving proper memory map with all regions to kernel
- [ ] Pass kernel config options onto kernel
- [ ] Don't create kernel heap in Seed (in conjunction with kernel changes)
- [ ] Decide if we want to boot via UEFI on RV64 too
- [ ] If not ^, finish VirtIO disk driver and FAT loading on RV64 Seed
- [ ] Retire use of ramdisk on RV64

## Kernel
- [ ] Facilitate early physical memory allocation from Seed memory map
- [ ] Create initial kernel heap in kernel instead of Seed
- [ ] Dynamically grow kernel heap as required
- [ ] Move to common logging framework with abstracted serial output
- [ ] Support early logging with strategy selection for early boot (strategy from run-time kernel config)
- [ ] Common kernel abstraction for HHDM - associated consts for address etc. on the `Platform` trait?
- [ ] Allow kernel to own one side of a `Channel` and respond to messages in `async` context
- [ ] Split ACPI initialization into table access and then later namespace initialization after we have a clocksource
- [x] Calibrate the TSC clocksource from the HPET if frequency is not reported via `cpuid`

## Ginkgo
- [ ] Get REPL working again with new bytecode VM
- [ ] Proper error handling so invalid input does not panic shell

# Ideas for future versions (use this to prevent scope-creep of v0.1)
- Rejigging of PlatformBus to support kernelspace bus drivers
- Pass PCI config space out to userspace via new PBus bus driver
- Dynamically extract Virtio PCI caps in GPU driver
- Support userspace reset of EHCI controller using PCI config space
- Create a GOP framebuffer object on the PBus (will be needed for real hardware booting)
- RTC driver for x86 (probs in kernelspace)
- EC driver
- ACPI shutdown
- ACPI suspend
- Support VirtIO GPU hardware cursor
- Compositor with windowing support, hardware compositing if available
- Support both windowing and raw FB in `fb_console`
- Get Poplar booting on Thinkpad
- Detect platforms with no invariant TSC support and use the HPET as a clocksource

### TODO: building Poplar on a new Debian machine
- Turns out you need a C toolchain... do `apt install build-essentials`
- `xtask` wants a bunch of stuff: `pkg-config`, `libudev` (why??)
- You need to also do `rustup component add rust-src`