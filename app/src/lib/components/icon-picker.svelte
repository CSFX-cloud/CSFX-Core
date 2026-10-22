<script lang="ts">
    import Icon from "@iconify/svelte";

    let {
        icon = $bindable(),
        color = $bindable(),
        imageUrl = null,
        uploadingImage = false,
        imageError = null,
        onUploadImage,
        onRemoveImage,
    }: {
        icon: string;
        color: string;
        imageUrl?: string | null;
        uploadingImage?: boolean;
        imageError?: string | null;
        onUploadImage?: (file: File) => void;
        onRemoveImage?: () => void;
    } = $props();

    let fileInput = $state<HTMLInputElement | null>(null);

    function handleFileChange(event: Event) {
        const file = (event.target as HTMLInputElement).files?.[0];
        if (file) onUploadImage?.(file);
        if (fileInput) fileInput.value = "";
    }

    const SUGGESTED_ICONS = [
        "mdi:cube-outline",
        "mdi:server",
        "mdi:database",
        "mdi:rocket-launch-outline",
        "mdi:shield-check-outline",
        "mdi:layers-outline",
        "mdi:cloud-outline",
        "mdi:flask-outline",
        "mdi:cog-outline",
        "mdi:web",
        "mdi:folder-outline",
        "mdi:fire",
    ];

    const SUGGESTED_COLORS = [
        "#6366f1",
        "#22c55e",
        "#ef4444",
        "#f59e0b",
        "#06b6d4",
        "#a855f7",
        "#ec4899",
        "#64748b",
    ];
</script>

<div class="md__look">
    <span class="tile tile--lg tint" style="--tint: {color}" aria-hidden="true">
        {#if imageUrl}
            <img src={imageUrl} alt="" />
        {:else}
            <Icon icon={icon || "mdi:cube-outline"} width={24} height={24} />
        {/if}
    </span>
    <div style="display: grid; gap: 16px; min-width: 0">
        <div>
            <p class="md-label">Icon</p>
            <div class="md-icons" role="group" aria-label="Icon">
                {#each SUGGESTED_ICONS as suggestion (suggestion)}
                    <button
                        type="button"
                        class:on={icon === suggestion}
                        aria-label={suggestion}
                        aria-pressed={icon === suggestion}
                        title={suggestion}
                        onclick={() => (icon = suggestion)}
                    >
                        <Icon icon={suggestion} width={16} height={16} />
                    </button>
                {/each}
            </div>
            <div class="cx-field" style="margin-top: 10px">
                <label class="cx-label" for="icon-name">Icon name</label>
                <input class="cx-input cx-mono" id="icon-name" placeholder="mdi:cube-outline" bind:value={icon} disabled={!!imageUrl} />
            </div>
        </div>

        {#if onUploadImage}
            <div>
                <input bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/svg+xml" class="db-sr-only" id="icon-image-upload" onchange={handleFileChange} />
                <div class="db-actions">
                    <label for="icon-image-upload" class="cx-btn cx-btn--secondary cx-btn--sm" style={uploadingImage ? "opacity: .5; pointer-events: none" : ""}>
                        {uploadingImage ? "Uploading" : imageUrl ? "Replace image" : "Upload image"}
                    </label>
                    {#if imageUrl}
                        <button type="button" class="cx-btn cx-btn--ghost cx-btn--sm nd-danger" onclick={() => onRemoveImage?.()}>Remove</button>
                    {/if}
                </div>
                {#if imageError}<p class="cx-hint cx-hint--error">{imageError}</p>{/if}
            </div>
        {/if}

        <div>
            <p class="md-label">Color</p>
            <div class="md-colors" role="group" aria-label="Color">
                <input type="color" aria-label="Custom color" bind:value={color} />
                {#each SUGGESTED_COLORS as suggestion (suggestion)}
                    <button
                        type="button"
                        class:on={color === suggestion}
                        style="background: {suggestion}"
                        aria-label={suggestion}
                        aria-pressed={color === suggestion}
                        onclick={() => (color = suggestion)}
                    ></button>
                {/each}
            </div>
        </div>
    </div>
</div>
