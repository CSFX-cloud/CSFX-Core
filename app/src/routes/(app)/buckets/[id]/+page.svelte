<script lang="ts">
    import { page } from "$app/stores";
    import { auth } from "$lib/auth/store.svelte";
    import {
        getBucket,
        listBucketObjects,
        deleteBucketObject,
        presignObjectUpload,
        presignObjectDownload,
        uploadObjectToPresignedUrl,
        type Bucket,
        type ObjectEntry,
    } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import Icon from "@iconify/svelte";

    const bucketId: string = $page.params.id;

    let bucket = $state<Bucket | null>(null);
    let objects = $state<ObjectEntry[]>([]);
    let folders = $state<string[]>([]);
    let currentPrefix = $state("");
    let loading = $state(true);
    let error = $state<string | null>(null);
    let uploading = $state(false);
    let fileInput = $state<HTMLInputElement | null>(null);

    function fmtBytes(bytes: number): string {
        if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
        if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
        if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
        return `${bytes} B`;
    }

    function folderName(prefix: string): string {
        const trimmed = prefix.slice(0, -1);
        const parts = trimmed.split("/");
        return parts[parts.length - 1];
    }

    function objectName(key: string): string {
        const parts = key.split("/");
        return parts[parts.length - 1];
    }

    function breadcrumbParts(): { name: string; prefix: string }[] {
        if (!currentPrefix) return [];
        const segments = currentPrefix.slice(0, -1).split("/");
        const parts: { name: string; prefix: string }[] = [];
        let acc = "";
        for (const segment of segments) {
            acc += `${segment}/`;
            parts.push({ name: segment, prefix: acc });
        }
        return parts;
    }

    async function load() {
        if (!auth.token) return;
        try {
            await Promise.all([
                getBucket(auth.token, bucketId).then((b) => (bucket = b)),
                loadObjects(),
            ]);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load bucket";
        } finally {
            loading = false;
        }
    }

    async function loadObjects() {
        if (!auth.token) return;
        const result = await listBucketObjects(auth.token, bucketId, currentPrefix);
        objects = result.objects;
        folders = result.folders;
    }

    function openFolder(prefix: string) {
        currentPrefix = prefix;
        loadObjects();
    }

    async function handleUpload(event: Event) {
        if (!auth.token) return;
        const input = event.target as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;

        uploading = true;
        error = null;
        try {
            const key = `${currentPrefix}${file.name}`;
            const presigned = await presignObjectUpload(auth.token, bucketId, key);
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
            const presigned = await presignObjectDownload(auth.token, bucketId, key);
            window.open(presigned.url, "_blank");
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to download file";
        }
    }

    async function handleDelete(key: string) {
        if (!auth.token) return;
        try {
            await deleteBucketObject(auth.token, bucketId, key);
            objects = objects.filter((o) => o.key !== key);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete object";
        }
    }

    let loadStarted = false;
    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            load();
        }
    });
</script>

<header class="flex h-16 shrink-0 items-center gap-2 px-4 border-b">
    <a href="/buckets" class="text-sm text-muted-foreground hover:text-foreground">S3 Buckets</a>
    <span class="text-sm text-muted-foreground">/</span>
    <span class="text-sm font-medium">{bucket?.name ?? bucketId.slice(0, 8)}</span>
</header>

<div class="flex flex-col gap-6 p-6">
    {#if loading}
        <p class="text-sm text-muted-foreground">Loading...</p>
    {:else if error && !bucket}
        <p class="text-sm text-destructive">{error}</p>
    {:else if bucket}
        <div class="flex items-center justify-between">
            <div>
                <h1 class="text-xl font-semibold tracking-tight">{bucket.name}</h1>
                <p class="text-sm text-muted-foreground mt-0.5 font-mono">{bucket.global_alias}</p>
            </div>
            <div>
                <input
                    bind:this={fileInput}
                    type="file"
                    class="hidden"
                    onchange={handleUpload}
                />
                <Button size="sm" onclick={() => fileInput?.click()} disabled={uploading}>
                    <Icon icon="mdi:upload" width={14} height={14} />
                    {uploading ? "Uploading..." : "Upload File"}
                </Button>
            </div>
        </div>

        <div class="flex items-center gap-1.5 text-sm">
            <button
                class="text-muted-foreground hover:text-foreground"
                onclick={() => openFolder("")}
            >
                {bucket.name}
            </button>
            {#each breadcrumbParts() as part}
                <span class="text-muted-foreground">/</span>
                <button
                    class="text-muted-foreground hover:text-foreground"
                    onclick={() => openFolder(part.prefix)}
                >
                    {part.name}
                </button>
            {/each}
        </div>

        {#if error}
            <p class="text-xs text-destructive">{error}</p>
        {/if}

        <div class="border rounded-lg overflow-hidden">
            <table class="w-full text-sm">
                <thead class="bg-muted/50">
                    <tr>
                        <th class="text-left px-4 py-3 font-medium text-muted-foreground">Name</th>
                        <th class="text-left px-4 py-3 font-medium text-muted-foreground">Size</th>
                        <th class="text-left px-4 py-3 font-medium text-muted-foreground">Last Modified</th>
                        <th class="text-right px-4 py-3 font-medium text-muted-foreground">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#if folders.length === 0 && objects.length === 0}
                        <tr>
                            <td colspan="4" class="px-4 py-8 text-center text-muted-foreground">
                                This folder is empty
                            </td>
                        </tr>
                    {:else}
                        {#each folders as folder (folder)}
                            <tr
                                class="border-t hover:bg-muted/30 transition-colors cursor-pointer"
                                onclick={() => openFolder(folder)}
                            >
                                <td class="px-4 py-3">
                                    <div class="flex items-center gap-2.5">
                                        <Icon icon="mdi:folder-outline" width={16} height={16} class="text-muted-foreground" />
                                        <span class="font-medium">{folderName(folder)}</span>
                                    </div>
                                </td>
                                <td class="px-4 py-3 text-muted-foreground">-</td>
                                <td class="px-4 py-3 text-muted-foreground">-</td>
                                <td class="px-4 py-3"></td>
                            </tr>
                        {/each}
                        {#each objects as object (object.key)}
                            <tr class="border-t hover:bg-muted/30 transition-colors">
                                <td class="px-4 py-3">
                                    <div class="flex items-center gap-2.5">
                                        <Icon icon="mdi:file-outline" width={16} height={16} class="text-muted-foreground" />
                                        <span>{objectName(object.key)}</span>
                                    </div>
                                </td>
                                <td class="px-4 py-3 text-muted-foreground">{fmtBytes(object.size)}</td>
                                <td class="px-4 py-3 text-muted-foreground text-xs">{object.last_modified}</td>
                                <td class="px-4 py-3">
                                    <div class="flex items-center justify-end gap-1">
                                        <button
                                            class="flex items-center justify-center w-7 h-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                                            onclick={() => handleDownload(object.key)}
                                            aria-label="Download"
                                            title="Download"
                                        >
                                            <Icon icon="mdi:download" width={16} height={16} />
                                        </button>
                                        <button
                                            class="flex items-center justify-center w-7 h-7 rounded-full text-destructive hover:bg-destructive/10 transition-colors"
                                            onclick={() => handleDelete(object.key)}
                                            aria-label="Delete"
                                            title="Delete"
                                        >
                                            <Icon icon="mdi:trash-can-outline" width={16} height={16} />
                                        </button>
                                    </div>
                                </td>
                            </tr>
                        {/each}
                    {/if}
                </tbody>
            </table>
        </div>
    {/if}
</div>
