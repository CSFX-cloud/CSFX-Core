<script lang="ts">
    import { untrack } from "svelte";
    import { RefreshCw } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { updateWorkload, type Workload } from "$lib/api/resource-groups";

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

<section class="cx-panel st-panel" aria-labelledby="ct-set">
    <div><h3 id="ct-set">Configuration</h3><p>Changes apply on the next redeploy.</p></div>
    <form
        class="rd-form"
        onsubmit={(event) => {
            event.preventDefault();
            redeploy();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="s-image">Image</label>
            <input class="cx-input cx-mono" id="s-image" placeholder="nginx:latest" autocomplete="off" bind:value={image} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="s-env">Environment variables</label>
            <textarea class="rd-textarea" id="s-env" placeholder="KEY=value" aria-describedby="s-env-hint" bind:value={envText}></textarea>
            <p class="cx-hint" id="s-env-hint">One KEY=value per line.</p>
        </div>
        <div class="rd-row2">
            <div class="cx-field">
                <label class="cx-label" for="s-restart-policy">Restart policy</label>
                <select class="cx-input" id="s-restart-policy" bind:value={restartPolicy}>
                    <option value="always">Always</option>
                    <option value="on-failure">On failure</option>
                    <option value="never">Never</option>
                </select>
            </div>
            <div class="cx-field">
                <label class="cx-label" for="s-max-restarts">Max restarts</label>
                <input class="cx-input" id="s-max-restarts" type="number" min="0" placeholder="Unlimited" bind:value={maxRestarts} />
            </div>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="rd-form-foot">
            <p>Redeploying recreates the container with this configuration and pulls the image again if it changed.</p>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving} aria-busy={saving}>
                <RefreshCw class="cx-i" aria-hidden="true" />
                {saving ? "Redeploying" : "Redeploy"}
            </button>
        </div>
    </form>
</section>
