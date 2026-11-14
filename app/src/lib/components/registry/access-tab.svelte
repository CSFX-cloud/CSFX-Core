<script lang="ts">
    import { Plus, Trash2 } from "@lucide/svelte";
    import { deleteToken, type RegistryDetail } from "$lib/api/registries";
    import CodeBlock from "$lib/components/resource-group/code-block.svelte";
    import { relativeTime } from "$lib/utils/dashboard";
    import TokenDialog from "./token-dialog.svelte";

    type Props = { rgId: string; registry: RegistryDetail; host: string; onChanged: () => Promise<void>; onError: (message: string) => void };

    let { rgId, registry, host, onChanged, onError }: Props = $props();

    let tokenDialog = $state<TokenDialog | null>(null);

    const usage = $derived(
        [
            `docker login ${host} -u <username> -p <secret>`,
            `docker tag my-app:latest ${host}/${registry.prefix}my-app:latest`,
            `docker push ${host}/${registry.prefix}my-app:latest`,
        ].join("\n"),
    );

    async function revoke(tokenId: string, name: string) {
        if (!confirm(`Revoke ${name}? Anything that uses it loses access immediately.`)) return;
        try {
            await deleteToken(rgId, registry.id, tokenId);
            await onChanged();
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to revoke access token");
        }
    }
</script>

<TokenDialog bind:this={tokenDialog} {rgId} registryId={registry.id} onCreated={onChanged} />

<section class="cx-panel" aria-labelledby="ra-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="ra-title">Access tokens</h2>
            <p class="cx-panel__sub">Tokens only reach images in {registry.prefix}</p>
        </div>
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" onclick={() => tokenDialog?.open()}>
            <Plus class="cx-i" aria-hidden="true" />
            New token
        </button>
    </div>
    <div class="db-scroll">
        <table class="cx-table">
            <thead><tr><th>Name</th><th>Username</th><th>Access</th><th>Last used</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
            <tbody>
                {#each registry.tokens as token (token.id)}
                    <tr>
                        <td>{token.name}</td>
                        <td class="cx-soft"><code>{token.username}</code></td>
                        <td class="cx-soft">{token.scope === "push" ? "Push and pull" : "Pull only"}</td>
                        <td class="cx-soft">{token.last_used_at ? relativeTime(token.last_used_at, Date.now()) : "never"}</td>
                        <td>
                            <div class="db-actions" style="justify-content: flex-end">
                                <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Revoke {token.name}" title="Revoke" onclick={() => revoke(token.id, token.name)}>
                                    <Trash2 class="cx-i" aria-hidden="true" />
                                </button>
                            </div>
                        </td>
                    </tr>
                {:else}
                    <tr><td colspan="5" class="cx-soft">No tokens yet.</td></tr>
                {/each}
            </tbody>
        </table>
    </div>
</section>

<CodeBlock filename="push an image" tag="shell" language="shell" code={usage} />
<p class="cx-hint">The registry speaks plain http inside your network. Add {host} to the insecure registries of your docker client until TLS is set up.</p>
