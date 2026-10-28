<script lang="ts">
    import type { Bucket } from "$lib/api/resource-groups";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";
    import { bucketEndpoint, bucketSnippets } from "$lib/utils/bucket-snippets";
    import CodeBlock from "./code-block.svelte";

    let { bucket }: { bucket: Bucket } = $props();

    let active = $state("aws-cli");

    const snippets = $derived(bucketSnippets(bucket, window.location.origin));
    const current = $derived(snippets.find((snippet) => snippet.id === active) ?? snippets[0]);
    const endpoint = $derived(bucketEndpoint(bucket, window.location.origin));
</script>

<section class="cx-panel st-panel" aria-labelledby="bi-endpoint">
    <div>
        <h3 id="bi-endpoint">Endpoint</h3>
        <p>
            {bucket.exposure === "external"
                ? "Public S3 endpoint for this bucket."
                : "Reachable from workloads inside the resource group. Set exposure to external in Settings for public access."}
        </p>
    </div>
    <p class="cx-mono" style="margin: 0; word-break: break-all">{endpoint}</p>
    <p class="cx-hint">Create an access key in Settings and replace the placeholders below.</p>
</section>

<WorkloadTabs tabs={snippets} {active} label="Integration examples" onselect={(id) => (active = id)} />

{#key current.id}
    <CodeBlock filename={current.filename} tag={current.tag} language={current.language} code={current.code} />
{/key}
