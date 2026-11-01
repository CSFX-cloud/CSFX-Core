<script lang="ts">
    import { Plus, Trash2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { addSiteDomain, deleteSiteDomain, type SiteDetail } from "$lib/api/sites";

    type Props = { rgId: string; site: SiteDetail; onChanged: () => Promise<void>; onError: (message: string) => void };

    let { rgId, site, onChanged, onError }: Props = $props();

    let hostname = $state("");
    let adding = $state(false);

    function pointsTo(kind: string, branch: string | null): string {
        if (kind === "branch") return `branch: ${branch ?? "unknown"}`;
        return "production";
    }

    async function run(action: (token: string) => Promise<unknown>, failure: string) {
        if (!auth.token) return;
        try {
            await action(auth.token);
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : failure);
        }
    }

    async function add() {
        if (hostname.trim() === "") return;
        adding = true;
        await run((token) => addSiteDomain(token, rgId, site.id, hostname.trim()), "Failed to add domain");
        hostname = "";
        adding = false;
    }
</script>

<section class="cx-panel" aria-labelledby="dm-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="dm-title">Domains</h2>
            <p class="cx-panel__sub">Point the DNS record of a custom domain at your gateway, then add it here.</p>
        </div>
    </div>
    <div class="db-scroll">
        <table class="cx-table">
            <thead><tr><th>Domain</th><th>Points to</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
            <tbody>
                {#each site.domains as domain (domain.id)}
                    <tr>
                        <td>{domain.hostname}</td>
                        <td class="cx-soft">{pointsTo(domain.kind, domain.branch)}</td>
                        <td>
                            <div class="db-actions" style="justify-content: flex-end">
                                <button
                                    class="cx-icon-btn cx-icon-btn--sm nd-danger"
                                    type="button"
                                    aria-label="Remove {domain.hostname}"
                                    title="Remove"
                                    onclick={() => run((token) => deleteSiteDomain(token, rgId, site.id, domain.id), "Failed to remove domain")}
                                >
                                    <Trash2 class="cx-i" aria-hidden="true" />
                                </button>
                            </div>
                        </td>
                    </tr>
                {:else}
                    <tr><td colspan="3" class="cx-soft">No domains yet.</td></tr>
                {/each}
            </tbody>
        </table>
    </div>
    <form
        style="display: flex; gap: 12px; align-items: end; padding: 16px 24px 24px"
        onsubmit={(event) => {
            event.preventDefault();
            add();
        }}
    >
        <div class="cx-field" style="flex: 1">
            <label class="cx-label" for="dm-host">Domain</label>
            <input class="cx-input" id="dm-host" placeholder="shop.example.com" autocomplete="off" bind:value={hostname} />
        </div>
        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="submit" disabled={hostname.trim() === "" || adding}>
            <Plus class="cx-i" aria-hidden="true" />
            Add
        </button>
    </form>
</section>
