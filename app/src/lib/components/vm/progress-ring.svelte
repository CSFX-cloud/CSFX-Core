<script lang="ts">
    let { fraction, size = 20 }: { fraction: number; size?: number } = $props();

    const STROKE = 3;
    const radius = $derived((size - STROKE) / 2);
    const circumference = $derived(2 * Math.PI * radius);
    const percent = $derived(Math.round(Math.min(Math.max(fraction, 0), 1) * 100));
</script>

<svg
    width={size}
    height={size}
    viewBox="0 0 {size} {size}"
    role="progressbar"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={percent}
    style="flex-shrink: 0; transform: rotate(-90deg)"
>
    <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke="currentColor" stroke-opacity="0.2" stroke-width={STROKE} />
    <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        stroke-width={STROKE}
        stroke-linecap="round"
        stroke-dasharray={circumference}
        stroke-dashoffset={circumference * (1 - percent / 100)}
        style="transition: stroke-dashoffset 0.15s linear"
    />
</svg>
