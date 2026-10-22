<script lang="ts">
    import { untrack } from "svelte";
    import { Download, File, Folder, FolderOpen, Trash2, Upload } from "@lucide/svelte";
    import { page } from "$app/stores";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteBucketObject,
        getBucket,
        listBucketObjects,
        presignObjectDownload,
        presignObjectUpload,
        uploadObjectToPresignedUrl,
        type Bucket,
        type ObjectEntry,
    } from "$lib/api/resource-groups";
    import EmptyState from "$lib/components/dashboard/empty-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import { fmtBytes, formatBytes } from "$lib/utils/format";
    import { folderName, objectName, prefixCrumbs } from "$lib/utils/bucket-objects";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    const SKELETON_ROWS = 3;

    const bucketId: string = $page.params.id ?? "";

    let bucket = $state<Bucket | null>(null);
    let objects = $state<ObjectEntry[]>([]);
    let folders = $state<string[]>([]);
    let currentPrefix = $state("");
    let loading = $state(true);
    let error = $state<string | null>(null);
    let uploading = $state(false);
    let fileInput = $state<HTMLInputElement | null>(null);

    const crumbs = $derived(prefixCrumbs(currentPrefix));
    const empty = $derived(folders.length === 0 && objects.length === 0);

    async function loadObjects() {
        if (!auth.token) return;
        const result = await listBucketObjects(auth.token, bucketId, currentPrefix);
        objects = result.objects;
        folders = result.folders;
    }

    async function load() {
        if (!auth.token) return;
        try {
            await Promise.all([getBucket(auth.token, bucketId).then((loaded) => (bucket = loaded)), loadObjects()]);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load bucket";
        } finally {
            loading = false;
        }
    }

    function openFolder(prefix: string) {
        currentPrefix = prefix;
        loadObjects().catch((e) => (error = e instanceof Error ? e.message : "Failed to load objects"));
    }

    async function handleUpload(event: Event) {
        const file = (event.target as HTMLInputElement).files?.[0];
        if (!auth.token || !file) return;
        uploading = true;
        error = null;
        try {
            const presigned = await presignObjectUpload(auth.token, bucketId, `${currentPrefix}${file.name}`);
            await uploadObjectToPresignedUrl(presigned.url, file);
            await loadObjects();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to upload file";
        } finally {
            uploading = false;
            if (fileInput) fileInput.value = "";
        }
    }

    async function handleDownload(key: string) {
        if (!auth.token) return;
        try {
            window.open((await presignObjectDownload(auth.token, bucketId, key)).url, "_blank");
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to download file";
        }
    }

    async function handleDelete(key: string) {
        if (!auth.token || !confirm(`Delete ${objectName(key)}?`)) return;
        try {
            await deleteBucketObject(auth.token, bucketId, key);
            objects = objects.filter((object) => object.key !== key);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete object";
        }
    }

    $effect(() => {
        if (auth.token) untrack(load);
    });

    $effect(() => {
        breadcrumb.set(bucket?.name ?? bucketId.slice(0, 8));
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    {#if loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !bucket}
        <p class="cx-hint cx-hint--error">{error ?? "Bucket not found"}</p>
    {:else}
        <div class="db-head">
            <div>
                <h1 class="db-title">{bucket.name}</h1>
                <p class="db-sub cx-mono" style="font-size: 13px">{bucket.global_alias}</p>
            </div>
            <div class="db-actions">
                <input bind:this={fileInput} type="file" class="db-sr-only" aria-label="Choose a file to upload" onchange={handleUpload} />
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="button" disabled={uploading} aria-busy={uploading} onclick={() => fileInput?.click()}>
                    <Upload class="cx-i" aria-hidden="true" />
                    {uploading ? "Uploading" : "Upload file"}
                </button>
            </div>
        </div>

        <section class="db-stats" aria-label="Bucket details">
            <div class="cx-stat">
                <p class="cx-stat__label">Status</p>
                <p class="db-stat-value"><span class={badgeClass(bucket.status)}>{statusLabel(bucket.status)}</span></p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Exposure</p>
                <p class="db-stat-value">{bucket.exposure}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Quota</p>
                <p class="db-stat-value">{bucket.quota_max_size ? fmtBytes(bucket.quota_max_size) : "Unlimited"}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Resource group</p>
                <p class="db-stat-value" style="font-size: 16px">
                    {#if bucket.resource_group_id}
                        <a class="cx-link" href="/resource-groups/{bucket.resource_group_id}">{bucket.resource_group_id.slice(0, 8)}</a>
                    {:else}
                        &mdash;
                    {/if}
                </p>
            </div>
        </section>

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <section class="cx-panel" aria-label="Objects">
            <nav class="bk-crumbs" aria-label="Folder path">
                <button class="bk-crumb" type="button" onclick={() => openFolder("")}>{bucket.name}</button>
                {#each crumbs as crumb (crumb.prefix)}
                    <span class="bk-crumbs__separator">/</span>
                    <button class="bk-crumb" type="button" onclick={() => openFolder(crumb.prefix)}>{crumb.name}</button>
                {/each}
            </nav>

            {#if empty}
                <EmptyState icon={FolderOpen} label="This folder is empty" />
            {:else}
                <div class="db-scroll">
                    <table class="cx-table">
                        <thead><tr><th>Name</th><th>Size</th><th>Last modified</th><th><span class="db-sr-only">Actions</span></th></tr></thead>
                        <tbody>
                            {#each folders as folder (folder)}
                                <tr class="nl-row">
                                    <td>
                                        <div class="rs-name">
                                            <span class="cx-tile-plain nl-tile" aria-hidden="true"><Folder class="cx-i" /></span>
                                            <button class="rs-link" type="button" onclick={() => openFolder(folder)}>{folderName(folder)}</button>
                                        </div>
                                    </td>
                                    <td class="cx-soft">&mdash;</td>
                                    <td class="cx-soft">&mdash;</td>
                                    <td></td>
                                </tr>
                            {/each}
                            {#each objects as object (object.key)}
                                <tr>
                                    <td>
                                        <div class="rs-name">
                                            <span class="cx-tile-plain nl-tile" aria-hidden="true"><File class="cx-i" /></span>
                                            <span>{objectName(object.key)}</span>
                                        </div>
                                    </td>
                                    <td class="cx-num">{formatBytes(object.size)}</td>
                                    <td class="cx-num">{object.last_modified}</td>
                                    <td>
                                        <div class="db-actions" style="justify-content: flex-end">
                                            <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Download {objectName(object.key)}" title="Download" onclick={() => handleDownload(object.key)}>
                                                <Download class="cx-i" aria-hidden="true" />
                                            </button>
                                            <button class="cx-icon-btn cx-icon-btn--sm nd-danger" type="button" aria-label="Delete {objectName(object.key)}" title="Delete" onclick={() => handleDelete(object.key)}>
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
        </section>
    {/if}
</div>
