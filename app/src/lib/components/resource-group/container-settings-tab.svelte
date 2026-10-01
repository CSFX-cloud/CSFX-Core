<script lang="ts">
    import { untrack } from "svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { updateWorkload, type Workload } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";

    let { workload, onRedeployed }: { workload: Workload; onRedeployed: () => Promise<void> } = $props();

    const initial = untrack(() => workload);

    let image = $state(initial.image);
    let envText = $state(
        Object.entries(initial.env_vars ?? {})
            .map(([key, value]) => `${key}=${value}`)
            .join("\n"),
    );
    let restartPolicy = $state(initial.restart_policy as "always" | "on-failure" | "never");
    let maxRestarts = $state(initial.max_restarts !== null ? String(initial.max_restarts) : "");
    let error = $state<string | null>(null);
    let saving = $state(false);

    function parseEnv(raw: string): Record<string, string> {
        const result: Record<string, string> = {};
        for (const line of raw.split("\n")) {
            const trimmed = line.trim();
            if (!trimmed) continue;
            const separator = trimmed.indexOf("=");
            if (separator === -1) throw new Error(`Invalid env var line: "${trimmed}" (expected KEY=VALUE)`);
            result[trimmed.slice(0, separator)] = trimmed.slice(separator + 1);
        }
        return result;
    }

    function parseMaxRestarts(raw: string): number | null {
        if (raw.trim() === "") return null;
        const value = Number(raw);
        if (!Number.isInteger(value) || value < 0) throw new Error("Max restarts must be a non-negative integer");
        return value;
    }

    async function redeploy() {
        if (!auth.token) return;
        saving = true;
        error = null;
        try {
            if (!image.trim()) throw new Error("Image cannot be empty");
            await updateWorkload(auth.token, workload.id, {
                image: image.trim(),
                env_vars: parseEnv(envText),
                restart_policy: restartPolicy,
                max_restarts: parseMaxRestarts(maxRestarts),
            });
            await onRedeployed();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to save settings";
        } finally {
            saving = false;
        }
    }
</script>

<div class="h-full overflow-y-auto p-6 space-y-5">
    <div class="flex flex-col gap-1">
        <label class="text-xs text-muted-foreground" for="s-image">Image</label>
        <input id="s-image" class="border rounded px-3 py-1.5 text-sm bg-background font-mono" placeholder="nginx:latest" bind:value={image} />
    </div>
    <div class="flex flex-col gap-1">
        <label class="text-xs text-muted-foreground" for="s-env">Environment variables</label>
        <textarea id="s-env" class="border rounded px-3 py-2 text-sm bg-background font-mono h-40 resize-y" placeholder="KEY=value" bind:value={envText}></textarea>
        <p class="text-xs text-muted-foreground">One KEY=VALUE per line.</p>
    </div>
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="s-restart-policy">Restart policy</label>
            <select id="s-restart-policy" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={restartPolicy}>
                <option value="always">Always</option>
                <option value="on-failure">On failure</option>
                <option value="never">Never</option>
            </select>
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="s-max-restarts">Max restarts</label>
            <input id="s-max-restarts" type="number" min="0" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="Unlimited" bind:value={maxRestarts} />
        </div>
    </div>
    <p class="text-xs text-muted-foreground">
        Redeploying applies the new configuration by recreating the container, pulling the image again if changed.
    </p>
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex justify-end">
        <Button size="sm" onclick={redeploy} disabled={saving}>
            {saving ? "Redeploying..." : "Redeploy"}
        </Button>
    </div>
</div>
