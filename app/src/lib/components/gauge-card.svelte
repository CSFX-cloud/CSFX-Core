<script lang="ts">
    const ARC_FRACTION = 0.7;
    const ROTATION_DEG = 90 + (360 * (1 - ARC_FRACTION)) / 2;
    const RADIUS = 44;
    const WARN_THRESHOLD = 60;
    const CRITICAL_THRESHOLD = 80;

    let { value, label, detail }: { value: number; label: string; detail: string } = $props();

    const circumference = 2 * Math.PI * RADIUS;
    const arcLength = circumference * ARC_FRACTION;

    const clamped = $derived(Math.min(100, Math.max(0, value)));
    const filledDasharray = $derived(`${((clamped / 100) * arcLength).toFixed(1)} ${circumference.toFixed(1)}`);
    const trackDasharray = `${arcLength.toFixed(1)} ${circumference.toFixed(1)}`;
    const color = $derived(
        clamped > CRITICAL_THRESHOLD ? "#ef4444" : clamped > WARN_THRESHOLD ? "#eab308" : "currentColor",
    );
</script>

<div class="p-3 flex flex-col items-center">
    <div class="relative shrink-0">
        <svg width="112" height="112" viewBox="0 0 112 112" style="color: {color}">
            <circle
                cx="56" cy="56" r={RADIUS} fill="none"
                stroke="currentColor" stroke-width="8" stroke-opacity="0.12"
                stroke-dasharray={trackDasharray}
                stroke-linecap="round"
                transform="rotate({ROTATION_DEG} 56 56)"
            />
            <circle
                cx="56" cy="56" r={RADIUS} fill="none"
                stroke="currentColor" stroke-width="8"
                stroke-dasharray={filledDasharray}
                stroke-linecap="round"
                transform="rotate({ROTATION_DEG} 56 56)"
            />
        </svg>
        <div class="absolute inset-0 flex items-center justify-center">
            <span class="text-xl font-semibold leading-none">{clamped.toFixed(0)}%</span>
        </div>
    </div>
    <div class="flex flex-col items-center gap-0.5 mt-1 text-center">
        <span class="text-xs font-medium leading-tight">{label}</span>
        <span class="text-[11px] text-muted-foreground truncate leading-tight max-w-full">{detail}</span>
    </div>
</div>
