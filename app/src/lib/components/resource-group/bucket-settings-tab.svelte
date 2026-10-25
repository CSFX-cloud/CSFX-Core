<script lang="ts">
    import { untrack } from "svelte";
    import { Trash2 } from "@lucide/svelte";
    import { auth } from "$lib/auth/store.svelte";
    import {
        createBucketKey,
        deleteBucketKey,
        listBucketKeys,
        updateBucket,
        type Bucket,
        type BucketAccessKey,
        type BucketAccessKeyCreated,
    } from "$lib/api/resource-groups";
    import { createClipboard } from "$lib/utils/clipboard.svelte";

    const BYTES_PER_GIB = 1024 ** 3;

    let { bucket, onUpdated }: { bucket: Bucket; onUpdated: (bucket: Bucket) => void } = $props();

    const clipboard = createClipboard();
    const initial = untrack(() => bucket);

    let keys = $state<BucketAccessKey[]>([]);
    let error = $state<string | null>(null);
    let newKeyName = $state("");
    let newKeyExpiry = $state("");
    let creating = $state(false);
    let createdKey = $state<BucketAccessKeyCreated | null>(null);
    let exposure = $state<"internal" | "external">(initial.exposure === "external" ? "external" : "internal");
    let quotaGib = $state(initial.quota_max_size ? String(initial.quota_max_size / BYTES_PER_GIB) : "");
    let maxObjects = $state(initial.quota_max_objects ? String(initial.quota_max_objects) : "");
    let saving = $state(false);
    let saved = $state(false);

    async function loadKeys() {
        if (!auth.token) return;
        try {
            keys = await listBucketKeys(auth.token, bucket.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load access keys";
        }
    }

    async function createKey() {
        if (!auth.token || !newKeyName) return;
        creating = true;
        error = null;
        try {
            createdKey = await createBucketKey(auth.token, bucket.id, {
                name: newKeyName,
                ...(newKeyExpiry && { expires_at: `${newKeyExpiry}T23:59:59` }),
            });
            newKeyName = "";
            newKeyExpiry = "";
            await loadKeys();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create access key";
        } finally {
            creating = false;
        }
    }

    async function removeKey(key: BucketAccessKey) {
        if (!auth.token || !confirm(`Delete access key ${key.name}?`)) return;
        error = null;
        try {
            await deleteBucketKey(auth.token, bucket.id, key.id);
            keys = keys.filter((entry) => entry.id !== key.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete access key";
        }
    }

    async function saveSettings() {
        if (!auth.token) return;
        saving = true;
        saved = false;
        error = null;
        try {
            onUpdated(
                await updateBucket(auth.token, bucket.id, {
                    exposure,
                    ...(quotaGib && { quota_max_size: Math.round(Number(quotaGib) * BYTES_PER_GIB) }),
                    ...(maxObjects && { quota_max_objects: Number(maxObjects) }),
                }),
            );
            saved = true;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to update bucket";
        } finally {
            saving = false;
        }
    }

    $effect(() => {
        if (auth.token) untrack(loadKeys);
    });
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<section class="cx-panel st-panel" aria-labelledby="bs-keys">
    <div><h3 id="bs-keys">Access keys</h3><p>Keys grant S3 access to this bucket.</p></div>
    {#if keys.length === 0}
        <p class="cx-hint">No access keys yet</p>
    {:else}
        <div class="db-scroll">
            <table class="cx-table">
                <thead><tr><th>Name</th><th>Key ID</th><th>Permissions</th><th>Expires</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
                <tbody>
                    {#each keys as key (key.id)}
                        <tr>
                            <td>{key.name}</td>
                            <td class="cx-num">{key.garage_key_id}</td>
                            <td class="cx-soft">{key.permissions}</td>
                            <td class="cx-soft">{key.expires_at ? key.expires_at.slice(0, 10) : "Never"}</td>
                            <td>
                                <div class="db-actions" style="justify-content: flex-end">
                                    <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Delete {key.name}" title="Delete" onclick={() => removeKey(key)}>
                                        <Trash2 class="cx-i" aria-hidden="true" />
                                    </button>
                                </div>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}

    {#if createdKey}
        <div class="bk-secret">
            <p style="margin: 0; font-weight: 500">Secret access key created</p>
            <p class="cx-mono" style="margin: 0; font-size: 12px; word-break: break-all">{createdKey.secret_access_key}</p>
            <p class="cx-hint cx-hint--error">This will not be shown again, copy it now.</p>
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" style="justify-self: start" onclick={() => clipboard.copy(createdKey?.secret_access_key ?? "", "bucket-secret")}>
                {clipboard.copiedKey === "bucket-secret" ? "Copied" : "Copy"}
            </button>
        </div>
    {/if}

    <form
        class="st-row"
        onsubmit={(event) => {
            event.preventDefault();
            createKey();
        }}
    >
        <input class="cx-input" placeholder="Key name" aria-label="Key name" autocomplete="off" bind:value={newKeyName} />
        <input class="cx-input" type="date" aria-label="Expiry date (optional)" title="Expiry date (optional)" style="flex: 0 0 160px" bind:value={newKeyExpiry} />
        <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" style="height: 44px" disabled={creating || !newKeyName} aria-busy={creating}>
            {creating ? "Creating" : "Create key"}
        </button>
    </form>
</section>

<section class="cx-panel st-panel" aria-labelledby="bs-config">
    <div><h3 id="bs-config">Configuration</h3><p>Exposure and storage limits.</p></div>
    <form
        class="rd-form"
        onsubmit={(event) => {
            event.preventDefault();
            saveSettings();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="bs-exposure">Exposure</label>
            <select class="cx-input" id="bs-exposure" bind:value={exposure}>
                <option value="internal">Internal (resource group only)</option>
                <option value="external">External (public endpoint)</option>
            </select>
        </div>
        <div class="rd-row2">
            <div class="cx-field">
                <label class="cx-label" for="bs-quota">Size quota (GiB)</label>
                <input class="cx-input" id="bs-quota" type="number" min="0" step="any" placeholder="Unlimited" bind:value={quotaGib} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="bs-objects">Object quota</label>
                <input class="cx-input" id="bs-objects" type="number" min="0" step="1" placeholder="Unlimited" bind:value={maxObjects} />
            </div>
        </div>
        <div class="rd-form-foot">
            <p>{saved ? "Saved" : "Quotas cannot be reset to unlimited once set."}</p>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving} aria-busy={saving}>
                {saving ? "Saving" : "Save settings"}
            </button>
        </div>
    </form>
</section>
