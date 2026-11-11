<script lang="ts">
    import { onMount } from "svelte";
    import { Check, Copy, Trash2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { deleteImage, listImages, type RegistryDetail, type Repository } from "$lib/api/registries";
    import { createClipboard } from "$lib/utils/clipboard.svelte";
    import { formatBytes } from "$lib/utils/format";

    const DIGEST_LENGTH = 19;

    type Props = { rgId: string; registry: RegistryDetail; host: string; onLoaded: (repositories: Repository[]) => void; onError: (message: string) => void };

    let { rgId, registry, host, onLoaded, onError }: Props = $props();

    const clipboard = createClipboard();

    let repositories = $state<Repository[]>([]);
    let loading = $state(true);

    const rows = $derived(
        repositories.flatMap((repository) =>
            repository.tags.map((tag) => ({
                repository: repository.repository,
                tag,
                reference: `${host}/${registry.prefix}${repository.repository}:${tag.tag}`,
            })),
        ),
    );

    async function load() {
        if (!auth.token) return;
        try {
            repositories = (await listImages(auth.token, rgId, registry.id)).repositories;
            onLoaded(repositories);
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to list images");
        } finally {
            loading = false;
        }
    }

    async function remove(repository: string, tag: string) {
        if (!auth.token || !confirm(`Delete ${repository}:${tag}? Deployments that still use it can no longer start.`)) return;
        try {
            await deleteImage(auth.token, rgId, registry.id, repository, tag);
            await load();
        } catch (e) {
            onError(e instanceof Error ? e.message : "Failed to delete image");
        }
    }

    onMount(load);
</script>

<section class="cx-panel" aria-labelledby="ri-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="ri-title">Images</h2>
            <p class="cx-panel__sub">Copy a reference to use it as the image of a container in this resource group.</p>
        </div>
    </div>
    <div class="db-scroll">
        <table class="cx-table">
            <thead><tr><th>Repository</th><th>Tag</th><th>Size</th><th>Digest</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
            <tbody>
                {#each rows as row (row.reference)}
                    <tr>
                        <td><span class="kv-name">{row.repository}</span></td>
                        <td>{row.tag.tag}</td>
                        <td class="cx-num">{row.tag.size_bytes === null ? "—" : formatBytes(row.tag.size_bytes)}</td>
                        <td class="cx-soft"><code>{row.tag.digest?.slice(0, DIGEST_LENGTH) ?? "—"}</code></td>
                        <td>
                            <div class="db-actions" style="justify-content: flex-end">
                                <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Copy reference of {row.repository}:{row.tag.tag}" title="Copy reference" onclick={() => clipboard.copy(row.reference, row.reference)}>
                                    {#if clipboard.copiedKey === row.reference}<Check class="cx-i" aria-hidden="true" />{:else}<Copy class="cx-i" aria-hidden="true" />{/if}
                                </button>
                                <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Delete {row.repository}:{row.tag.tag}" title="Delete" onclick={() => remove(row.repository, row.tag.tag)}>
                                    <Trash2 class="cx-i" aria-hidden="true" />
                                </button>
                            </div>
                        </td>
                    </tr>
                {:else}
                    <tr><td colspan="5" class="cx-soft">{loading ? "Loading" : "No images yet. Create an access token and push your first image."}</td></tr>
                {/each}
            </tbody>
        </table>
    </div>
</section>
