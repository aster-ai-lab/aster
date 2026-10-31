# 其它平台还要完成的硬件虚拟机工作

本文从 OS `main` 的 `f1078a74` 起算。共享配置已经在仓库里。Linux KVM（x86_64 与 aarch64）和 Windows x86_64 WHPX 还要在各自的机器上做完下面的实机项。本文不把任何主机记成已通过，也不翻转 `ROADMAP.md` 的 W3、R2 或 R2L。

Apple Silicon 上一次本地 HVF 运行只说明这台 Mac 能创建虚拟机并进入一个 Linux 客户机。它不能代替 KVM 或 WHPX，也不能把 HVF 的就绪级别提升为 fresh-host。

## 已经共享、各平台不要再写一套

编码器在 `crates/oci-runtime/crates/core/src/hardware_vm.rs`。Box 的产品规则在 `crates/box/src/runtime/src/hardware_vm.rs`，通过 path 依赖使用同一份配置字节。

| 项 | 三台机器共用的值 |
| --- | --- |
| 配置版本 | `1` |
| 配置字节 | SHA-256 `56f8192e40be87270ff34fcef11a824445eeb79a3bd5ce22af0e70b282015d21`，长度 207。前 4 字节是 `01 00 00 00` |
| 字节里允许出现的内容 | 版本、设备角色、生命周期、快照层、vCPU 与内存界限。字节里不写 hypervisor 名称，也不写客户机 ISA |
| vCPU | `1`、`2`、`4`、`8` |
| 内存 | 128 到 8192 MiB，按 128 MiB 对齐 |
| 设备 | `virtio-mmio-blk`、`virtio-mmio-console`、`virtio-mmio-fs`、`virtio-mmio-net`、`virtio-mmio-vsock` |
| 生命周期 | `provision`、`start`、`exec`、`stop`、`deprovision` |
| 快照层名称 | `platform`、`workload-start`、`instance-checkpoint` |
| 启动布局 | `linux-direct-x86_64` 与 `linux-direct-aarch64` 各一份。布局名称不进配置字节 |
| 控制通道限额 | 单条记录最多 65536 字节，信用窗口 32 |
| 省略 `--isolation` | 选择 hardware-vm。显式 `--isolation sandbox` 拒绝上面这些只属于虚拟机的特性。显式 `microvm` 拼写继续被拒绝 |

`VmConfig::new` 先调用 `validate_profile_resources`。Windows x86_64 在这之后仍要求恰好 1 个 vCPU，报错文本保持 `exactly 1 vCPU`。2、4、8 在共享配置里合法，只在这块 WHPX 主机门槛上失败。

virtio-fs 共享令牌：KVM 与 HVF 是 `same-uid-private-per-generation-virtiofs`，WHPX 是 `protected-per-generation-virtiofs`。Native Linux 没有这份硬件虚拟机共享令牌。

检查点：`(LibkrunKvm|LibkrunHvf|LibkrunWhpx, DedicatedVm|SharedGuestKernel)` 允许；`(NativeLinux, SharedHostKernel)` 保持原样。libkrun 的客户机架构只接受 `x86_64` 和 `aarch64`。Native Linux 的 CRIU 检查点继续接受原来的架构令牌，包括 `amd64`。不要用这三层快照替换 Native Linux CRIU。

客户机内核输入在 `crates/box/src/runtime/guest-kernel/{x86_64,aarch64}/`。每个 ISA 有一份 config、按文件名排序的补丁，以及 `SOURCE-DIGEST`。`verify_kernel_source` 核对这三样。当前记录的摘要是：

- x86_64：`826bb6907e02654b74c0e59455cd5630e71e05acae3a40e3bbb230421ec953d2`
- aarch64：`d6505eb0f5263e41fe3a9e3e8675c8664e116e6d15248c4b1b303b001d6d74b3`

这些文件是契约输入，不是已经编出来的 KVM 或 WHPX 客户机内核。

## 这台 Mac 已经留下的范围

2026-10-04，arm64，`kern.hv_support=1`。二进制按 `packaging/macos/aster-oci-hvf.entitlements` 做了 ad-hoc 签名。三次命令都退出 0：

1. `aster-oci hvf-smoke`，schema `aster.oci.hvf-smoke.v1`。它调用 `hv_vm_create` / `hv_vm_destroy`，不读取这份配置。
2. `aster-oci-krun-shim context-smoke`，schema `aster.oci.krun-context-smoke.v2`。`VmConfig::new(1, 128)`，没有进入客户机。
3. `aster-oci-krun-shim vm-smoke`，schema `aster.oci.krun-vm-smoke.v2`。`VmConfig::new(1, 512)`，客户机退出码 0，根盘只读，runtime share 与根盘分开，nonce 标记核对后删除。

同日交叉编译 `x86_64-unknown-linux-gnu` 与 `x86_64-pc-windows-msvc` 的 `cargo check -p aster-oci-runtime --lib` 通过。两次都没有启动客户机。`aarch64-unknown-linux-gnu` 没有做这次检查。Windows 单测 `rejects_uncertified_windows_smp_configuration` 带有 `#[cfg(all(target_os = "windows", target_arch = "x86_64"))]`，没有在这台 Mac 上执行。

没有记录任何一组 launch-to-runnable 毫秒数。测试里的 40、100、70 只是夹具，不能写进实机报告。临时客户机镜像已经删除，Linux 和 Windows 使用各自文档里的 system-image manifest。

## Linux KVM x86_64

在一台有 `/dev/kvm` 的 x86_64 Linux 上，从 `crates/oci-runtime` 用 rustc 1.98.1 完成：

1. `cargo test -p aster-oci-core --lib hardware_vm -- --test-threads=1`
2. `cargo test -p aster-oci-krun --lib -- rejects_resources_outside_the_shared_hardware_vm_profile accepts_profile_vcpu_and_aligned_memory`
3. `cargo test -p aster-oci-sdk --lib checkpoint::tests -- --test-threads=1`
4. 在 `crates/box/src` 里：`cargo test -p aster-box-runtime --lib hardware_vm -- --test-threads=1`，以及 `cargo test -p aster-box-cli --lib hardware_vm_consumer -- --test-threads=1`

然后跑实机门，命令和通过条件以 `docs/linux-native.md` 为准：

```bash
cargo run -p aster-oci-krun --bin aster-oci-krun-shim -- context-smoke
```

成功报告是 `aster.oci.krun-context-smoke.v2`，platform `linux`，1 vCPU，128 MiB。这一步不打开 `/dev/kvm`，也不进入客户机。

真正进入客户机使用短的 `RUNNER_TEMP`（例如 `/tmp`）和本机的 system-image manifest：

```bash
export RUNNER_TEMP=/tmp
ASTER_OCI_LINUX_KVM_SYSTEM_IMAGE_MANIFEST=/absolute/path/to/system-image.json \
  bash .github/scripts/linux-kvm-agent-entry.sh
```

保留 `aster.oci.linux-kvm-agent-entry.v1`。客户机 ISA 用 `linux-direct-x86_64`，内核输入对上 x86_64 的 `SOURCE-DIGEST`。根盘只读。目录共享只接受同一 UID；外来 UID 在 KVM 上失败。控制通道是同一用户的 Unix socket，限额用共享配置里的 65536 和 32。

fresh-host 晋升仍走 `ROADMAP.md` 的 Linux KVM R2L：`ASTER_OCI_LINUX_KVM_HOST_CLASS=fresh`，加上 `aster.oci.linux-kvm-fresh-host-attestation.v1`，soak 保持 25 轮。用过的机器和 WSL 保持 `existing`，报告里 `promotes_readiness` 维持 false。

## Linux KVM aarch64

重复上一节的全部命令和报告，换到 aarch64 Linux 主机。客户机 ISA 用 `linux-direct-aarch64`，内核输入对上 aarch64 的 `SOURCE-DIGEST`。x86_64 的通过结果不能抄过来。

这台 Mac 的 aarch64 客户机是 HVF，不是 KVM。KVM aarch64 要自己打开 `/dev/kvm`、核对 API version 12，并留下 `aster.oci.linux-kvm-agent-entry.v1`。

## Windows x86_64 WHPX

在 Windows x86_64 上，从 `crates/oci-runtime` 用 rustc 1.98.1 完成：

1. 上一节里的 core、sdk、Box 三组测试。
2. krun 测试必须实际执行 `rejects_uncertified_windows_smp_configuration`。`VmConfig::new(2, 128)` 失败，文本包含 `exactly 1 vCPU`。`VmConfig::new(1, 128)` 与 `VmConfig::new(1, 512)` 成功。`(4, 8192)` 和 `(8, 128)` 也要被这道主机门槛拒绝。
3. `rejects_resources_outside_the_shared_hardware_vm_profile`：`(3, 256)`、`(1, 64)`、`(1, 129)`、`(1, 8193)` 失败，文本包含 `hardware-vm profile`。

实机命令和资产以 `docs/windows-whpx.md` 为准：`aster-oci whpx-smoke`、`aster-oci-krun-shim context-smoke`、`aster-oci-krun-shim vm-smoke`，以及文档里的 `agent-vm-smoke` 和 `oci-vm-smoke`。进入客户机时保持 1 vCPU、512 MiB、只读根盘，以及 `protected-per-generation-virtiofs`。客户机 ISA 是 x86_64。WHPX 不把 aarch64 算作合格主机。

外来 UID 只允许 portable metadata。guest-native ext4 和直接的外来 UID 目录共享都要失败。控制通道是同一用户的命名管道，记录长度和信用窗口与 KVM、HVF 相同。

fresh-host 晋升仍走 `ROADMAP.md` 的 WHPX R2：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\windows-whpx-release-matrix.ps1 `
  -RootfsArchive <alpine-minirootfs.tar.gz> `
  -SystemImageManifest <system-image.json> `
  -HostClass fresh `
  -FreshHostAttestation <fresh-host-attestation.json>
```

已经跑过 WHPX 的机器保持 `-HostClass existing`。

## 三个主机都还要把产品规则接到启动路径

下面这些函数只在 `crates/box/src/runtime/src/hardware_vm.rs` 和它的单测里。OCI 的启动、检查点和驱动目前只调用了配置字节、资源准入、检查点 pair、客户机架构和 virtio-fs 令牌。KVM、HVF、WHPX 都要把下列行为接到真实启动上，并用该主机的报告留证：

1. 根身份按角色名加摘要识别 distro、runtime、custom，另加一个 ext4 scratch。识别不看主机磁盘枚举顺序。只读 EROFS 层加 ext4 scratch 的 overlay 还不是 OCI 启动用的根；CRI 和 krun shim 现在仍把容器根经 virtio-fs 送进客户机。
2. virtio-fs 只挂主机目录。共享不进快照字节。恢复时只有保存的路径、模式和拒绝列表都一致才重新挂上。
3. 快照三层的含义保持：`platform` 是克隆、新 scratch、只作为跨租户预镜像；`workload-start` 是克隆、配对、按摘要绑定租户；`instance-checkpoint` 是恢复、配对、一次性。发布先写临时目录，再做一次不覆盖的 rename。恢复中断时拆掉这台虚拟机。捕获后源虚拟机继续运行。快照字节里没有主机文件描述符。一种 hypervisor 的快照不能在另一种上恢复。热的工作负载内存不跨租户共享。
4. 客户机里的 supervisor 保持是工作负载的父进程。cgroup、namespace 和 capability 丢弃是客户机 Linux 的策略。
5. `provision`、`start`、`exec`、`stop`、`deprovision` 按结构、能力、确定性策略、本机 hypervisor 或工件的顺序拒绝。被拒绝的请求不写状态。
6. exec 结果只有 `exited`、`signaled`、`timed-out`、`cancelled`、`failed`、`indeterminate`。
7. 恢复网络时摘要必须一致，恢复块设备时角色必须一致。
8. 启动凭证不出现在 argv、环境、日志和快照字节里。拆除报告只有 schema 版本、不透明 id、类别和计数。
9. 每个主机记录自己的 launch-to-runnable 毫秒序列。这个特性只有在 KVM、HVF、WHPX 三组序列都存在、并且都落在同一个门限内时才可用。门限数值还没有从实机定下来。缺任何一组，特性就保持不可用。

`--isolation sandbox` 在三台机器上都拒绝第 1 到第 9 项。

## 一个平台怎样算做完

同时满足下面五条，才算该平台做完这份契约的剩余工作：

1. 该平台一节里的测试和实机命令在那台机器上退出 0，报告带上源提交、镜像或运行时资产摘要、平台、架构、驱动、隔离类和 schema。
2. 该平台的外来 UID 规则和控制通道端点在实机上按上面的区分执行。
3. 上一节的九条产品规则有该主机的启动路径证据，而不只是 Box 单测。
4. 该主机留下一组真实的 launch-to-runnable 毫秒数。三组没齐之前，不把延迟特性标成可用。
5. 就绪晋升另外保留 fresh-host 的 R2 或 R2L 矩阵。用过的开发机只产生 `existing` 观察，不把 `promotes_readiness` 设成 true。
