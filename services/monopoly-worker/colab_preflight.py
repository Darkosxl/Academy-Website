"""Print remotely measured Colab RAM, cgroup CPU quota, and machine shape as JSON."""

import json
import os
from pathlib import Path
import sys


def ram_bytes():
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemTotal:"):
            return int(line.split()[1]) * 1024
    return 0


def effective_vcpus():
    affinity = getattr(os, "sched_getaffinity", None)
    cpus = float(len(affinity(0)) if affinity else (os.cpu_count() or 0))
    try:
        quota, period = Path("/sys/fs/cgroup/cpu.max").read_text().split()
        if quota != "max":
            cpus = min(cpus, int(quota) / int(period))
    except (OSError, ValueError):
        try:
            quota = int(Path("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").read_text())
            period = int(Path("/sys/fs/cgroup/cpu/cpu.cfs_period_us").read_text())
            if quota > 0 and period > 0:
                cpus = min(cpus, quota / period)
        except (OSError, ValueError):
            pass
    return cpus


def machine_shape():
    values = []
    for path in [
        Path("/sys/devices/virtual/dmi/id/product_name"),
        Path("/sys/devices/virtual/dmi/id/product_version"),
    ]:
        try:
            value = path.read_text().strip()
            if value and value not in values:
                values.append(value)
        except OSError:
            pass
    if os.environ.get("COLAB_RELEASE_TAG"):
        values.append(f"colab:{os.environ['COLAB_RELEASE_TAG']}")
    return " · ".join(values) or sys.platform


print(
    "EXPOSURE_PREFLIGHT="
    + json.dumps(
        {
            "ram_bytes": ram_bytes(),
            "effective_vcpus": effective_vcpus(),
            "machine_shape": machine_shape(),
            "python": sys.version.split()[0],
        },
        separators=(",", ":"),
    )
)
