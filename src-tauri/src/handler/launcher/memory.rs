use sysinfo::System;

/// 根据物理内存和当前可用内存，给 JVM 留出系统和启动器所需的余量。
///
/// `-Xmx` 是上限而非建议值。尤其配合 `-XX:+AlwaysPreTouch` 时，设置为
/// 机器全部内存会在 JVM 初始化阶段立即占满内存，Linux 可能直接由 OOM
/// killer 终止进程，并在启动器中只表现为不明确的 `-1` 退出码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SafeMemoryLimit {
    Unknown,
    Insufficient { min_available_mb: u64 },
    Limited(u64),
}

pub(super) fn safe_max_memory_mb(
    requested_mb: u64,
    total_mb: u64,
    available_mb: u64,
) -> SafeMemoryLimit {
    if total_mb == 0 || available_mb == 0 {
        return SafeMemoryLimit::Unknown;
    }

    // 至少给桌面、启动器和原生库保留 512MB；内存较大的设备则保留总内存的 1/8，
    // 上限为 2GB，避免游戏占用所有物理内存。
    let reserve_mb = (total_mb / 8).clamp(512, 2048);
    let total_limit_mb = total_mb.saturating_mul(3) / 4;
    let available_limit_mb = available_mb.saturating_sub(reserve_mb);
    let limit_mb = total_limit_mb.min(available_limit_mb);

    if limit_mb < 512 {
        return SafeMemoryLimit::Insufficient {
            min_available_mb: reserve_mb + 512,
        };
    }

    SafeMemoryLimit::Limited(requested_mb.min(limit_mb))
}

pub(super) fn resolve_max_memory_mb(max_memory: &str) -> anyhow::Result<(u64, Option<String>)> {
    let requested_mb = max_memory
        .trim()
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("最大内存必须是一个有效的 MB 数值，当前值为: {max_memory}"))?;

    if requested_mb < 512 {
        return Err(anyhow::anyhow!(
            "最大内存不能低于 512MB，当前值为: {requested_mb}MB"
        ));
    }

    let mut system = System::new();
    system.refresh_memory();
    let total_mb = system.total_memory() / 1024 / 1024;
    let available_mb = system.available_memory() / 1024 / 1024;

    let effective_mb = match safe_max_memory_mb(requested_mb, total_mb, available_mb) {
        SafeMemoryLimit::Unknown => return Ok((requested_mb, None)),
        SafeMemoryLimit::Insufficient { min_available_mb } => {
            return Err(anyhow::anyhow!(
                "当前可用内存不足，至少需要约 {min_available_mb}MB 可用内存后再启动游戏。（系统总内存 {total_mb}MB，当前可用 {available_mb}MB）"
            ));
        }
        SafeMemoryLimit::Limited(effective_mb) => effective_mb,
    };

    let warning = (effective_mb < requested_mb).then(|| {
        format!(
            "已将最大内存从 {requested_mb}MB 调整为 {effective_mb}MB（系统总内存 {total_mb}MB，当前可用 {available_mb}MB），以避免系统内存不足导致游戏启动失败。"
        )
    });
    Ok((effective_mb, warning))
}

pub(super) fn is_heap_size_argument(argument: &str) -> bool {
    argument.starts_with("-Xmx")
        || argument.starts_with("-Xms")
        || argument.starts_with("-Xmn")
        || argument.starts_with("-XX:MaxHeapSize=")
        || argument.starts_with("-XX:InitialHeapSize=")
        || argument.starts_with("-XX:NewSize=")
        || argument.starts_with("-XX:MaxNewSize=")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_heap_size_argument_matches_known_heap_flags() {
        for arg in [
            "-Xmx4096m",
            "-Xms512m",
            "-Xmn128m",
            "-XX:MaxHeapSize=4294967296",
            "-XX:InitialHeapSize=536870912",
            "-XX:NewSize=134217728",
            "-XX:MaxNewSize=268435456",
        ] {
            assert!(is_heap_size_argument(arg), "{arg}");
        }
    }

    #[test]
    fn is_heap_size_argument_rejects_unrelated_flags() {
        for arg in ["-XX:+UseG1GC", "-Dfoo=bar", "", "-server", "-Xlog:gc"] {
            assert!(!is_heap_size_argument(arg), "{arg}");
        }
    }

    #[test]
    fn safe_max_memory_mb_reports_unknown_when_stats_are_missing() {
        assert_eq!(safe_max_memory_mb(2048, 0, 1024), SafeMemoryLimit::Unknown);
        assert_eq!(safe_max_memory_mb(2048, 4096, 0), SafeMemoryLimit::Unknown);
    }

    #[test]
    fn safe_max_memory_mb_applies_512mb_reserve_floor_and_flags_insufficient_memory() {
        // total=2048MB -> total/8=256MB, clamped up to the 512MB floor.
        // total_limit = 2048*3/4 = 1536; available_limit = 1000-512 = 488 (<512) => Insufficient.
        assert_eq!(
            safe_max_memory_mb(1024, 2048, 1000),
            SafeMemoryLimit::Insufficient {
                min_available_mb: 1024
            }
        );
    }

    #[test]
    fn safe_max_memory_mb_clamps_reserve_to_a_2gb_ceiling_on_large_machines() {
        // total=32768MB -> total/8=4096MB, clamped down to the 2048MB ceiling.
        // available_limit = 20000-2048 = 17952, which becomes the binding limit.
        assert_eq!(
            safe_max_memory_mb(8192, 32768, 20000),
            SafeMemoryLimit::Limited(8192)
        );
    }

    #[test]
    fn safe_max_memory_mb_never_exceeds_the_computed_limit_even_for_huge_requests() {
        assert_eq!(
            safe_max_memory_mb(999_999, 32768, 20000),
            SafeMemoryLimit::Limited(17952)
        );
    }

    #[test]
    fn safe_max_memory_mb_treats_exactly_512mb_limit_as_still_usable() {
        // total_limit = 683*3/4 = 512 (integer division); reserve is clamped to the 512MB floor.
        assert_eq!(
            safe_max_memory_mb(1024, 683, 10_000),
            SafeMemoryLimit::Limited(512)
        );
    }

    #[test]
    fn safe_max_memory_mb_treats_511mb_limit_as_insufficient() {
        // total_limit = 681*3/4 = 510 (<512) -> Insufficient.
        assert_eq!(
            safe_max_memory_mb(1024, 681, 10_000),
            SafeMemoryLimit::Insufficient {
                min_available_mb: 1024
            }
        );
    }

    #[test]
    fn resolve_max_memory_mb_rejects_non_numeric_input() {
        let err = resolve_max_memory_mb("not-a-number").unwrap_err();
        assert!(err.to_string().contains("最大内存必须是一个有效的 MB 数值"));
    }

    #[test]
    fn resolve_max_memory_mb_rejects_values_below_512mb() {
        let err = resolve_max_memory_mb("256").unwrap_err();
        assert!(err.to_string().contains("不能低于 512MB"));
    }

    #[test]
    fn resolve_max_memory_mb_trims_whitespace_before_parsing() {
        // 前后空白应被忽略；512 是允许的最小值，因此错误（如果有）一定不是解析错误，
        // 只可能是依赖真实系统内存状态得出的"内存不足"错误。
        match resolve_max_memory_mb("  512  ") {
            Ok((effective_mb, _)) => assert!(effective_mb > 0),
            Err(e) => assert!(!e.to_string().contains("必须是一个有效的 MB 数值")),
        }
    }
}
