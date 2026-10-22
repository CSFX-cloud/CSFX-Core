<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import {
        listBucketKeys,
        createBucketKey,
        type Bucket,
        type BucketAccessKey,
    } from "$lib/api/resource-groups";
    import { createClipboard } from "$lib/utils/clipboard.svelte";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId }: { rgId: string } = $props();

    const clipboard = createClipboard();

    let dialog = $state<ModalDialog | null>(null);
    let bucket = $state<Bucket | null>(null);
    let keys = $state<BucketAccessKey[]>([]);
    let error = $state<string | null>(null);
    let newKeyName = $state("");
    let creating = $state(false);
    let createdSecret = $state<string | null>(null);

    const endpoint = $derived(
        !bucket
            ? ""
            : bucket.exposure === "external"
              ? `${window.location.origin}/api/s3/${bucket.global_alias}`
              : `http://s3.svc.${rgId}.internal:3900`,
    );

    export async function open(target: Bucket) {
        if (!auth.token) return;
        bucket = target;
        error = null;
        createdSecret = null;
        newKeyName = "";
        dialog?.open();
        try {
            keys = await listBucketKeys(auth.token, target.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load access keys";
        }
    }

    async function createKey() {
        if (!auth.token || !bucket || !newKeyName) return;
        creating = true;
        error = null;
        try {
            const created = await createBucketKey(auth.token, bucket.id, { name: newKeyName });
            createdSecret = created.secret_access_key;
            newKeyName = "";
            keys = await listBucketKeys(auth.token, bucket.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create access key";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog
    bind:this={dialog}
    title={bucket?.name ?? ""}
    subtitle={endpoint}
    size="lg"
    onclose={() => {
        bucket = null;
        createdSecret = null;
    }}
>
    <div class="md__fields">
        <p class="md-label" style="margin: 0">Access keys</p>
        {#if keys.length === 0}
            <p class="cx-hint">No access keys yet</p>
        {:else}
            <div class="db-scroll">
                <table class="cx-table">
                    <thead><tr><th>Name</th><th>Key ID</th><th>Permissions</th></tr></thead>
                    <tbody>
                        {#each keys as key (key.id)}
                            <tr>
                                <td>{key.name}</td>
                                <td class="cx-num">{key.garage_key_id}</td>
                                <td class="cx-soft">{key.permissions}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}

        {#if createdSecret}
            <div class="bk-secret">
                <p style="margin: 0; font-weight: 500">Secret access key created</p>
                <p class="cx-mono" style="margin: 0; font-size: 12px; word-break: break-all">{createdSecret}</p>
                <p class="cx-hint cx-hint--error">This will not be shown again, copy it now.</p>
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" style="justify-self: start" onclick={() => clipboard.copy(createdSecret ?? "", "bucket-secret")}>
                    {clipboard.copiedKey === "bucket-secret" ? "Copied" : "Copy"}
                </button>
            </div>
        {/if}

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <form
            class="st-row"
            onsubmit={(event) => {
                event.preventDefault();
                createKey();
            }}
        >
            <input class="cx-input" placeholder="Key name" aria-label="Key name" autocomplete="off" bind:value={newKeyName} />
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" style="height: 44px" disabled={creating || !newKeyName} aria-busy={creating}>
                {creating ? "Creating" : "Create key"}
            </button>
        </form>
    </div>
</ModalDialog>
