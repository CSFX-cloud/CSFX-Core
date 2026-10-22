export function fmtBytes(bytes: number): string {
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(0)} MB`;
    return `${bytes} B`;
}

const BYTES_PER_GB = 1_073_741_824;
const GB_PER_TB = 1024;

export function formatCapacity(bytes: number | null | undefined): string {
    if (bytes == null) return "-";
    const gigabytes = bytes / BYTES_PER_GB;
    if (gigabytes >= GB_PER_TB) return `${(gigabytes / GB_PER_TB).toFixed(1)} TB`;
    return `${gigabytes.toFixed(1)} GB`;
}

const BYTES_PER_KB = 1024;
const BYTES_PER_MB = 1_048_576;

export function formatBytes(bytes: number | null | undefined): string {
    if (bytes == null) return "-";
    if (bytes >= BYTES_PER_GB) return `${(bytes / BYTES_PER_GB).toFixed(1)} GB`;
    if (bytes >= BYTES_PER_MB) return `${(bytes / BYTES_PER_MB).toFixed(1)} MB`;
    return `${(bytes / BYTES_PER_KB).toFixed(1)} KB`;
}

export function formatRate(bytesPerSecond: number): string {
    if (bytesPerSecond >= BYTES_PER_GB) return `${(bytesPerSecond / BYTES_PER_GB).toFixed(1)} GB/s`;
    if (bytesPerSecond >= BYTES_PER_MB) return `${(bytesPerSecond / BYTES_PER_MB).toFixed(1)} MB/s`;
    if (bytesPerSecond >= BYTES_PER_KB) return `${(bytesPerSecond / BYTES_PER_KB).toFixed(1)} KB/s`;
    return `${bytesPerSecond.toFixed(0)} B/s`;
}

const SECONDS_PER_DAY = 86_400;
const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_MINUTE = 60;

export function formatUptime(seconds: number | null | undefined): string {
    if (seconds == null) return "-";
    const days = Math.floor(seconds / SECONDS_PER_DAY);
    const hours = Math.floor((seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR);
    const minutes = Math.floor((seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
    if (days > 0) return `${days}d ${hours}h ${minutes}m`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m`;
}

export function formatPercent(value: number | null | undefined): string {
    return value == null ? "-" : `${value.toFixed(1)}%`;
}

export function pluralize(count: number, noun: string): string {
    return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
