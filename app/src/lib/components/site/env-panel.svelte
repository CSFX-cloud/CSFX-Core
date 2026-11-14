<script lang="ts">
    import { Plus, Trash2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { addSiteEnv, deleteSiteEnv, type EnvScope, type SiteDetail } from "$lib/api/sites";
    import { listSecrets } from "$lib/api/vaults";

    const MASK = "•".repeat(8);

    type Props = { rgId: string; site: SiteDetail; onChanged: () => Promise<void>; onError: (message: string) => void };

    let { rgId, site, onChanged, onError }: Props = $props();

    let name = $state("");
    let value = $state("");
    let secret = $state("");
    let scope = $state<EnvScope>("all");
    let secretNames = $state<string[]>([]);
    let adding = $state(false);

    const fromVault = $derived(secret !== "");
    const valid = $derived(name.trim() !== "" && (fromVault || value !== ""));

    $effect(() => {
        const vaultId = site.vault_id;
        if (!vaultId || !auth.token) return;
        listSecrets(rgId, vaultId)
            .then((secrets) => (secretNames = secrets.filter((entry) => !entry.deleted).map((entry) => entry.name)))
            .catch(() => (secretNames = []));
    });

    async function run(action: () => Promise<unknown>, failure: string) {
        try {
            await action();
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : failure);
        }
    }

    async function add() {
        if (!valid) return;
        adding = true;
        await run(
            () => addSiteEnv(rgId, site.id, { name: name.trim(), value: fromVault ? undefined : value, vault_secret: fromVault ? secret : undefined, scope }),
            "Failed to add variable",
        );
        name = "";
        value = "";
        secret = "";
        adding = false;
    }
</script>

<section class="cx-panel" aria-labelledby="ep-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="ep-title">Environment variables</h2>
            <p class="cx-panel__sub">
                {site.vault_id ? "Values can come from the key vault. They are read when a build starts." : "Select a key vault in the web app to read secrets from."}
            </p>
        </div>
    </div>
    <div class="db-scroll">
        <table class="cx-table">
            <thead><tr><th>Name</th><th>Value</th><th>Used in</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
            <tbody>
                {#each site.env as variable (variable.id)}
                    <tr>
                        <td><span class="kv-name">{variable.name}</span></td>
                        <td class="cx-soft">{variable.vault_secret ? `${MASK} from vault: ${variable.vault_secret}` : variable.value}</td>
                        <td class="cx-soft">{variable.scope}</td>
                        <td>
                            <div class="db-actions" style="justify-content: flex-end">
                                <button
                                    class="cx-icon-btn cx-icon-btn--sm nd-danger"
                                    type="button"
                                    aria-label="Remove {variable.name}"
                                    title="Remove"
                                    onclick={() => run(() => deleteSiteEnv(rgId, site.id, variable.id), "Failed to remove variable")}
                                >
                                    <Trash2 class="cx-i" aria-hidden="true" />
                                </button>
                            </div>
                        </td>
                    </tr>
                {:else}
                    <tr><td colspan="4" class="cx-soft">No variables yet.</td></tr>
                {/each}
            </tbody>
        </table>
    </div>
    <form
        style="display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; align-items: end; padding: 16px 24px 24px"
        onsubmit={(event) => {
            event.preventDefault();
            add();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="ep-name">Name</label>
            <input class="cx-input" id="ep-name" placeholder="PUBLIC_API_URL" autocomplete="off" bind:value={name} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="ep-secret">From vault</label>
            <select class="cx-input" id="ep-secret" bind:value={secret} disabled={secretNames.length === 0}>
                <option value="">Plain value</option>
                {#each secretNames as secretName (secretName)}<option value={secretName}>{secretName}</option>{/each}
            </select>
        </div>
        <div class="cx-field">
            <label class="cx-label" for="ep-value">Value</label>
            <input class="cx-input" id="ep-value" autocomplete="off" disabled={fromVault} bind:value={value} />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="ep-scope">Used in</label>
            <select class="cx-input" id="ep-scope" bind:value={scope}>
                <option value="all">All</option>
                <option value="production">Production</option>
                <option value="preview">Preview</option>
            </select>
        </div>
        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="submit" disabled={!valid || adding}>
            <Plus class="cx-i" aria-hidden="true" />
            Add
        </button>
    </form>
</section>
